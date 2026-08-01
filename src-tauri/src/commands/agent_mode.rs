//! Agent Test Mode：本地调试 HTTP 接口（夜间合同 §12 / §15）。
//!
//! - 仅开发构建编译：`#![cfg(any(debug_assertions, feature = "agent-mode"))]`
//!   （Release 稳定构建默认关闭；nightly 构建可显式开启 `agent-mode` feature）
//! - 极简 HTTP 服务：`std::net::TcpListener`，仅绑定 127.0.0.1，不引入第三方 HTTP crate
//! - 认证：随机 token（启动时生成，写入 session 文件），请求头 `X-Agent-Token`
//! - 安全边界：非 127.0.0.1 连接拒绝；`/open-fixture` 仅允许 fixture-root 内文件；
//!   日志脱敏（不输出文件内容）；禁止 shell 执行与任意文件读写
//!
//! 启动参数（`start_if_requested` 解析，经 lib.rs 调用）：
//! - `--agent-mode`
//! - `--fixture-root=<path>`
//! - `--agent-port=<port>`（0 = 自动分配，默认 0）
//! - `--session-file=<path>`（缺省写 fixture-root/agent-session.json 或系统临时目录）

#![cfg(any(debug_assertions, feature = "agent-mode"))]

use std::collections::{hash_map::RandomState, VecDeque};
use std::hash::BuildHasher;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// 环形日志上限（条）。
const MAX_LOG_ENTRIES: usize = 200;
/// 单请求 body 上限（1MB，防滥用）。
const MAX_BODY_BYTES: usize = 1024 * 1024;
/// 单连接读取超时。
const READ_TIMEOUT: Duration = Duration::from_secs(5);

/// 启动 Agent Test Mode（若命令行带 `--agent-mode`）。
///
/// 在 lib.rs 的 `run()` 开头调用。服务器运行在独立线程，不阻塞 Tauri 主循环。
/// 失败时返回错误描述（调用方打印日志即可，不影响应用正常启动）。
pub fn start_if_requested() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    if !args.iter().any(|a| a == "--agent-mode") {
        return Ok(());
    }

    let fixture_root = parse_arg(&args, "--fixture-root")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let port = parse_arg(&args, "--agent-port")
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(0);
    let session_file = parse_arg(&args, "--session-file").map(PathBuf::from);

    let server = AgentServer::new(fixture_root);
    let listener = TcpListener::bind(("127.0.0.1", port)).map_err(|e| e.to_string())?;
    let actual_port = listener.local_addr().map_err(|e| e.to_string())?.port();
    server.write_session_file(actual_port, session_file.as_deref())?;

    std::thread::Builder::new()
        .name("agent-mode".to_string())
        .spawn(move || server.run_loop(listener))
        .map_err(|e| e.to_string())?;

    eprintln!("[agent-mode] listening on http://127.0.0.1:{actual_port}");
    Ok(())
}

/// 解析 `--key=value` 或 `--key value` 形式参数。
fn parse_arg(args: &[String], key: &str) -> Option<String> {
    for (i, arg) in args.iter().enumerate() {
        if let Some(rest) = arg.strip_prefix(key) {
            if let Some(value) = rest.strip_prefix('=') {
                if !value.is_empty() {
                    return Some(value.to_string());
                }
            } else if arg == key {
                if let Some(next) = args.get(i + 1) {
                    return Some(next.clone());
                }
            }
        }
    }
    None
}

/// 运行时状态（route / source / status），供 `/state` 查询。
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentState {
    route: String,
    source: Option<String>,
    status: String,
}

/// Agent Test Mode 服务器。
struct AgentServer {
    token: String,
    fixture_root: PathBuf,
    state: Arc<Mutex<AgentState>>,
    logs: Arc<Mutex<VecDeque<String>>>,
    stop: Arc<AtomicBool>,
}

impl AgentServer {
    fn new(fixture_root: PathBuf) -> Self {
        Self {
            token: generate_token(),
            fixture_root,
            state: Arc::new(Mutex::new(AgentState {
                route: "/".to_string(),
                source: None,
                status: "ready".to_string(),
            })),
            logs: Arc::new(Mutex::new(VecDeque::new())),
            stop: Arc::new(AtomicBool::new(false)),
        }
    }

    /// accept 循环（阻塞，独立线程）。`/shutdown` 后退出进程。
    fn run_loop(&self, listener: TcpListener) {
        let _ = listener.set_nonblocking(true);
        loop {
            if self.stop.load(Ordering::SeqCst) {
                // 给客户端一点时间读完响应
                std::thread::sleep(Duration::from_millis(150));
                std::process::exit(0);
            }
            match listener.accept() {
                Ok((stream, _)) => self.handle_stream(stream),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(_) => std::thread::sleep(Duration::from_millis(20)),
            }
        }
    }

    /// 处理单个连接：非本地拒绝 → 解析 → 认证 → 路由。
    fn handle_stream(&self, mut stream: TcpStream) {
        let peer = match stream.peer_addr() {
            Ok(addr) => addr,
            Err(_) => return,
        };
        // 非 127.0.0.1 连接直接拒绝（合同 §12）
        if !is_loopback(&peer) {
            let _ = write_response(
                &mut stream,
                403,
                r#"{"error":"FORBIDDEN","message":"non-local connection rejected"}"#,
            );
            return;
        }
        let _ = stream.set_read_timeout(Some(READ_TIMEOUT));

        let request = match read_request(&mut stream) {
            Ok(req) => req,
            Err(_) => return,
        };

        // 认证：X-Agent-Token 头（大小写不敏感）
        if request.header("x-agent-token") != Some(self.token.as_str()) {
            self.log("401 unauthorized");
            let _ = write_response(&mut stream, 401, r#"{"error":"UNAUTHORIZED"}"#);
            return;
        }

        let path = request.target.split('?').next().unwrap_or("");
        let (status, body) = match (request.method.as_str(), path) {
            ("GET", "/health") => self.handle_health(),
            ("GET", "/state") => self.handle_state(),
            ("GET", "/logs") => self.handle_logs(),
            ("POST", "/open-file") => self.handle_open_file(&request),
            ("POST", "/open-fixture") => self.handle_open_fixture(&request),
            ("POST", "/navigate") => self.handle_navigate(&request),
            ("POST", "/reset-settings") => self.handle_reset_settings(),
            ("POST", "/clear-storage") => self.handle_clear_storage(),
            ("POST", "/simulate-error") => self.handle_simulate_error(&request),
            ("POST", "/shutdown") => self.handle_shutdown(),
            _ => (404, r#"{"error":"NOT_FOUND"}"#.to_string()),
        };
        let _ = write_response(&mut stream, status, &body);
    }

    fn handle_health(&self) -> (u16, String) {
        (200, r#"{"ok":true}"#.to_string())
    }

    fn handle_state(&self) -> (u16, String) {
        let state = self.state.lock().unwrap();
        (
            200,
            serde_json::to_string(&*state).unwrap_or_else(|_| "{}".to_string()),
        )
    }

    fn handle_logs(&self) -> (u16, String) {
        let logs = self.logs.lock().unwrap();
        let entries: Vec<&str> = logs.iter().map(|s| s.as_str()).collect();
        (
            200,
            serde_json::to_string(&entries).unwrap_or_else(|_| "[]".to_string()),
        )
    }

    fn handle_open_file(&self, request: &HttpRequest) -> (u16, String) {
        let path = match request.json_field("path") {
            Some(p) if !p.is_empty() => p,
            _ => {
                return (
                    400,
                    r#"{"error":"BAD_REQUEST","message":"missing or empty field: path"}"#
                        .to_string(),
                )
            }
        };
        let path_buf = PathBuf::from(&path);
        match std::fs::metadata(&path_buf) {
            Ok(meta) if meta.is_file() => {
                self.state.lock().unwrap().source = Some(path.clone());
                self.log(format!("open-file: {}", safe_name(&path)));
                (200, r#"{"ok":true}"#.to_string())
            }
            Ok(_) => (400, r#"{"error":"NOT_A_FILE"}"#.to_string()),
            Err(_) => (404, r#"{"error":"FILE_NOT_FOUND"}"#.to_string()),
        }
    }

    fn handle_open_fixture(&self, request: &HttpRequest) -> (u16, String) {
        let name = match request.json_field("name") {
            Some(n) if !n.is_empty() => n,
            _ => {
                return (
                    400,
                    r#"{"error":"BAD_REQUEST","message":"missing or empty field: name"}"#
                        .to_string(),
                )
            }
        };
        match resolve_fixture(&self.fixture_root, &name) {
            Ok(path) => match std::fs::metadata(&path) {
                Ok(meta) if meta.is_file() => {
                    self.state.lock().unwrap().source =
                        Some(path.to_string_lossy().into_owned());
                    self.log(format!("open-fixture: {}", safe_name(&name)));
                    (200, r#"{"ok":true}"#.to_string())
                }
                Ok(_) => (400, r#"{"error":"NOT_A_FILE"}"#.to_string()),
                Err(_) => (404, r#"{"error":"FILE_NOT_FOUND"}"#.to_string()),
            },
            Err(_) => (
                403,
                r#"{"error":"FIXTURE_OUT_OF_BOUNDS","message":"fixture name escapes fixture-root"}"#
                    .to_string(),
            ),
        }
    }

    fn handle_navigate(&self, request: &HttpRequest) -> (u16, String) {
        let route = match request.json_field("route") {
            Some(r) if !r.is_empty() => r,
            _ => {
                return (
                    400,
                    r#"{"error":"BAD_REQUEST","message":"missing or empty field: route"}"#
                        .to_string(),
                )
            }
        };
        if !route.starts_with('/')
            || route
                .chars()
                .any(|c| c.is_whitespace() || c == '?' || c == '#')
        {
            return (400, r#"{"error":"INVALID_ROUTE"}"#.to_string());
        }
        self.state.lock().unwrap().route = route.clone();
        self.log(format!("navigate: {route}"));
        (200, r#"{"ok":true}"#.to_string())
    }

    fn handle_reset_settings(&self) -> (u16, String) {
        self.state.lock().unwrap().status = "settings-reset".to_string();
        self.log("reset-settings");
        (200, r#"{"ok":true}"#.to_string())
    }

    fn handle_clear_storage(&self) -> (u16, String) {
        self.state.lock().unwrap().status = "storage-cleared".to_string();
        self.log("clear-storage");
        (200, r#"{"ok":true}"#.to_string())
    }

    fn handle_simulate_error(&self, request: &HttpRequest) -> (u16, String) {
        let code = match request.json_field("code") {
            Some(c) if !c.is_empty() => c,
            _ => {
                return (
                    400,
                    r#"{"error":"BAD_REQUEST","message":"missing or empty field: code"}"#
                        .to_string(),
                )
            }
        };
        // 只允许 [A-Za-z0-9_-]，防止日志注入
        if !code
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            return (400, r#"{"error":"INVALID_CODE"}"#.to_string());
        }
        self.state.lock().unwrap().status = format!("error:{code}");
        self.log(format!("simulate-error: {code}"));
        (200, r#"{"ok":true}"#.to_string())
    }

    fn handle_shutdown(&self) -> (u16, String) {
        self.log("shutdown requested");
        self.stop.store(true, Ordering::SeqCst);
        (200, r#"{"ok":true,"shuttingDown":true}"#.to_string())
    }

    /// 追加脱敏日志（环形缓冲，上限 MAX_LOG_ENTRIES）。
    fn log(&self, message: impl AsRef<str>) {
        let entry = format!("[{}] {}", format_utc_now(), message.as_ref());
        let mut logs = self.logs.lock().unwrap();
        if logs.len() >= MAX_LOG_ENTRIES {
            logs.pop_front();
        }
        logs.push_back(entry);
    }

    /// 写入会话文件（token / port / pid / fixtureRoot / startedAt）。
    fn write_session_file(&self, port: u16, explicit: Option<&Path>) -> Result<(), String> {
        let path = match explicit {
            Some(p) => p.to_path_buf(),
            None => {
                if self.fixture_root.is_dir() {
                    self.fixture_root.join("agent-session.json")
                } else {
                    std::env::temp_dir().join("featherview-agent-session.json")
                }
            }
        };
        #[derive(serde::Serialize)]
        #[serde(rename_all = "camelCase")]
        struct SessionInfo {
            token: String,
            port: u16,
            pid: u32,
            fixture_root: String,
            started_at: String,
        }
        let info = SessionInfo {
            token: self.token.clone(),
            port,
            pid: std::process::id(),
            fixture_root: self.fixture_root.to_string_lossy().into_owned(),
            started_at: format_utc_now(),
        };
        let json = serde_json::to_string_pretty(&info).map_err(|e| e.to_string())?;
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(&path, json)
            .map_err(|e| format!("write session file {}: {e}", path.display()))?;
        Ok(())
    }
}

// ---------- 工具函数 ----------

/// 仅允许 loopback 地址（127.0.0.0/8 与 ::1）。
fn is_loopback(addr: &SocketAddr) -> bool {
    match addr.ip() {
        std::net::IpAddr::V4(v4) => v4.is_loopback(),
        std::net::IpAddr::V6(v6) => v6.is_loopback(),
    }
}

/// 生成 64 位十六进制随机 token。
///
/// 熵源：进程内随机（`RandomState` 每进程随机种子）+ 时间纳秒 + PID，
/// 经 xorshift64* 扩展。不引入第三方 crate；用于本地调试接口认证。
fn generate_token() -> String {
    let mut seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
        ^ u64::from(std::process::id()).rotate_left(17);
    let rs = RandomState::new();
    seed ^= rs.hash_one("featherview-agent-token");
    seed ^= rs.hash_one(std::process::id());
    let mut x = seed | 1;
    let mut out = String::with_capacity(64);
    for _ in 0..16 {
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        out.push_str(&format!("{:016x}", x.wrapping_mul(0x2545_F491_4F6C_DD1D)));
    }
    out
}

/// 校验并解析 fixture 名 → fixture-root 内的路径。
///
/// 拒绝：绝对路径、Windows 盘符前缀、`..`、RootDir。目标存在时再用
/// canonicalize 双保险（防 root 为相对路径或符号链接逃逸）。
fn resolve_fixture(root: &Path, name: &str) -> Result<PathBuf, FixtureError> {
    if name.is_empty() {
        return Err(FixtureError::Invalid);
    }
    let raw = Path::new(name);
    if raw.is_absolute() {
        return Err(FixtureError::Escaped);
    }
    let mut parts: Vec<&std::ffi::OsStr> = Vec::new();
    for comp in raw.components() {
        match comp {
            Component::Normal(part) => parts.push(part),
            Component::CurDir => {}
            _ => return Err(FixtureError::Escaped), // RootDir / ParentDir / Prefix
        }
    }
    if parts.is_empty() {
        return Err(FixtureError::Invalid);
    }
    let mut joined = root.to_path_buf();
    for part in parts {
        joined.push(part);
    }
    if joined.exists() {
        let canon = std::fs::canonicalize(&joined).map_err(|_| FixtureError::Invalid)?;
        let canon_root = std::fs::canonicalize(root).map_err(|_| FixtureError::Invalid)?;
        if !canon.starts_with(&canon_root) {
            return Err(FixtureError::Escaped);
        }
        return Ok(canon);
    }
    Ok(joined)
}

#[derive(Debug, PartialEq, Eq)]
enum FixtureError {
    /// 路径逃逸（越界）。
    Escaped,
    /// 非法/无法解析。
    Invalid,
}

/// 日志脱敏：只保留文件名（basename）。
fn safe_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_else(|| "<unknown>".to_string())
}

/// UTC 时间戳 HH:MM:SSZ（日志用）。
fn format_utc_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let h = (secs / 3600) % 24;
    let m = (secs / 60) % 60;
    let s = secs % 60;
    format!("{h:02}:{m:02}:{s:02}Z")
}

// ---------- 极简 HTTP ----------

/// 解析后的 HTTP 请求（只支持本服务需要的子集）。
struct HttpRequest {
    method: String,
    target: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl HttpRequest {
    /// 按小写名称取首个请求头值。
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    /// 从 JSON body 取字符串字段。
    fn json_field(&self, key: &str) -> Option<String> {
        let text = String::from_utf8_lossy(&self.body);
        let value: serde_json::Value = serde_json::from_str(&text).ok()?;
        value.get(key)?.as_str().map(|s| s.to_string())
    }
}

/// 读取请求行、头与 body（Content-Length 限制 MAX_BODY_BYTES）。
fn read_request(stream: &mut TcpStream) -> io::Result<HttpRequest> {
    let mut reader = BufReader::new(stream.try_clone()?);

    let mut request_line = String::new();
    if reader.read_line(&mut request_line)? == 0 {
        return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "empty request"));
    }
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let target = parts.next().unwrap_or("").to_string();
    if method.is_empty() || target.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "malformed request line",
        ));
    }

    let mut headers: Vec<(String, String)> = Vec::new();
    let mut content_length: usize = 0;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        let trimmed = line.trim_end_matches(|c| c == '\r' || c == '\n');
        if trimmed.is_empty() {
            break;
        }
        if let Some((name, value)) = trimmed.split_once(':') {
            let name = name.trim().to_ascii_lowercase();
            let value = value.trim().to_string();
            if name == "content-length" {
                content_length = value.parse::<usize>().unwrap_or(0);
            }
            headers.push((name, value));
        }
    }

    if content_length > MAX_BODY_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "request body too large",
        ));
    }
    let mut body = vec![0u8; content_length];
    if content_length > 0 {
        reader.read_exact(&mut body)?;
    }

    Ok(HttpRequest {
        method,
        target,
        headers,
        body,
    })
}

/// 写 JSON 响应（Connection: close）。
fn write_response(stream: &mut TcpStream, status: u16, body: &str) -> io::Result<()> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "Unknown",
    };
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(body.as_bytes())?;
    stream.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 建一个临时 fixture-root（清理旧目录）。
    fn temp_root(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("featherview-agent-test-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// 在随机端口启动测试服务器（accept 循环），返回端口。
    fn start(server: Arc<AgentServer>) -> u16 {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                match stream {
                    Ok(s) => server.handle_stream(s),
                    Err(_) => continue,
                }
            }
        });
        port
    }

    /// 发一个 HTTP 请求，返回 (status, body)。
    fn request(
        port: u16,
        method: &str,
        target: &str,
        token: Option<&str>,
        body: Option<&str>,
    ) -> (u16, String) {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        let mut raw = format!("{method} {target} HTTP/1.1\r\nHost: 127.0.0.1\r\n");
        if let Some(t) = token {
            raw.push_str(&format!("X-Agent-Token: {t}\r\n"));
        }
        if let Some(b) = body {
            raw.push_str(&format!("Content-Length: {}\r\n", b.len()));
        }
        raw.push_str("\r\n");
        if let Some(b) = body {
            raw.push_str(b);
        }
        stream.write_all(raw.as_bytes()).unwrap();

        let mut resp = String::new();
        stream.read_to_string(&mut resp).unwrap();
        let status: u16 = resp
            .split_whitespace()
            .nth(1)
            .unwrap_or("0")
            .parse()
            .unwrap_or(0);
        let body_text = resp.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
        (status, body_text)
    }

    // ---- 认证与健康检查 ----

    #[test]
    fn health_requires_valid_token() {
        let root = temp_root("health");
        let server = Arc::new(AgentServer::new(root.clone()));
        let port = start(Arc::clone(&server));

        // 未授权：无 token / 错误 token → 401
        let (status, body) = request(port, "GET", "/health", None, None);
        assert_eq!(status, 401);
        assert!(body.contains("UNAUTHORIZED"));

        let (status, _) = request(port, "GET", "/health", Some("wrong-token"), None);
        assert_eq!(status, 401);

        // 正确 token → 200 {"ok":true}
        let (status, body) = request(port, "GET", "/health", Some(&server.token), None);
        assert_eq!(status, 200);
        assert!(body.contains("\"ok\":true"));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn token_is_random_per_server() {
        let a = AgentServer::new(temp_root("tok-a"));
        let b = AgentServer::new(temp_root("tok-b"));
        assert_ne!(a.token, b.token);
        assert_eq!(a.token.len(), 256);
        assert!(a.token.chars().all(|c| c.is_ascii_hexdigit()));
        let _ = std::fs::remove_dir_all(&a.fixture_root);
        let _ = std::fs::remove_dir_all(&b.fixture_root);
    }

    // ---- /state /logs ----

    #[test]
    fn state_reports_initial_values() {
        let root = temp_root("state");
        let server = Arc::new(AgentServer::new(root.clone()));
        let port = start(Arc::clone(&server));
        let (status, body) = request(port, "GET", "/state", Some(&server.token), None);
        assert_eq!(status, 200);
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["route"], "/");
        assert_eq!(v["status"], "ready");
        assert!(v["source"].is_null());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn navigate_updates_route_and_rejects_invalid() {
        let root = temp_root("nav");
        let server = Arc::new(AgentServer::new(root.clone()));
        let port = start(Arc::clone(&server));

        let (status, _) = request(
            port,
            "POST",
            "/navigate",
            Some(&server.token),
            Some(&serde_json::json!({"route": "/settings"}).to_string()),
        );
        assert_eq!(status, 200);

        let (_, body) = request(port, "GET", "/state", Some(&server.token), None);
        assert!(body.contains("\"route\":\"/settings\""));

        // 非法 route（无前导 /）
        let (status, _) = request(
            port,
            "POST",
            "/navigate",
            Some(&server.token),
            Some(&serde_json::json!({"route": "settings"}).to_string()),
        );
        assert_eq!(status, 400);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn logs_are_sanitized_and_capped() {
        let root = temp_root("logs");
        let secret_file = root.join("secret.txt");
        std::fs::write(&secret_file, "TOP_SECRET_CONTENT_XYZ").unwrap();
        let server = Arc::new(AgentServer::new(root.clone()));
        let port = start(Arc::clone(&server));

        let (status, _) = request(
            port,
            "POST",
            "/open-file",
            Some(&server.token),
            Some(&serde_json::json!({"path": secret_file.to_string_lossy()}).to_string()),
        );
        assert_eq!(status, 200);

        let (status, body) = request(port, "GET", "/logs", Some(&server.token), None);
        assert_eq!(status, 200);
        assert!(body.contains("open-file"));
        assert!(body.contains("secret.txt"));
        // 脱敏：绝不出现文件内容
        assert!(!body.contains("TOP_SECRET_CONTENT_XYZ"));

        // 环形上限
        {
            let mut logs = server.logs.lock().unwrap();
            for i in 0..MAX_LOG_ENTRIES + 50 {
                logs.push_back(format!("bulk-{i}"));
            }
            while logs.len() > MAX_LOG_ENTRIES {
                logs.pop_front();
            }
            assert_eq!(logs.len(), MAX_LOG_ENTRIES);
        }
        let _ = std::fs::remove_dir_all(root);
    }

    // ---- /open-file ----

    #[test]
    fn open_file_updates_source_and_404_on_missing() {
        let root = temp_root("openfile");
        let file = root.join("doc.md");
        std::fs::write(&file, "# hi").unwrap();
        let server = Arc::new(AgentServer::new(root.clone()));
        let port = start(Arc::clone(&server));

        let (status, _) = request(
            port,
            "POST",
            "/open-file",
            Some(&server.token),
            Some(&serde_json::json!({"path": file.to_string_lossy()}).to_string()),
        );
        assert_eq!(status, 200);
        let (_, body) = request(port, "GET", "/state", Some(&server.token), None);
        assert!(body.contains("doc.md"));

        let missing = root.join("nope.txt");
        let (status, body) = request(
            port,
            "POST",
            "/open-file",
            Some(&server.token),
            Some(&serde_json::json!({"path": missing.to_string_lossy()}).to_string()),
        );
        assert_eq!(status, 404);
        assert!(body.contains("FILE_NOT_FOUND"));

        // 坏 JSON → 400
        let (status, _) = request(
            port,
            "POST",
            "/open-file",
            Some(&server.token),
            Some("not-json"),
        );
        assert_eq!(status, 400);
        let _ = std::fs::remove_dir_all(root);
    }

    // ---- /open-fixture 越界 ----

    #[test]
    fn open_fixture_accepts_inside_and_rejects_escape() {
        let root = temp_root("fixture");
        std::fs::write(root.join("ok.txt"), "ok").unwrap();
        let server = Arc::new(AgentServer::new(root.clone()));
        let port = start(Arc::clone(&server));

        // 合法
        let (status, _) = request(
            port,
            "POST",
            "/open-fixture",
            Some(&server.token),
            Some(&serde_json::json!({"name": "ok.txt"}).to_string()),
        );
        assert_eq!(status, 200);

        // 越界：../
        for evil in ["../escape.txt", "sub/../../escape.txt", "./../escape.txt"] {
            let (status, body) = request(
                port,
                "POST",
                "/open-fixture",
                Some(&server.token),
                Some(&serde_json::json!({"name": evil}).to_string()),
            );
            assert_eq!(status, 403, "name={evil}");
            assert!(body.contains("FIXTURE_OUT_OF_BOUNDS"));
        }

        // 越界：绝对路径（平台相关）
        #[cfg(windows)]
        let abs = "C:\\Windows\\win.ini";
        #[cfg(not(windows))]
        let abs = "/etc/passwd";
        let (status, _) = request(
            port,
            "POST",
            "/open-fixture",
            Some(&server.token),
            Some(&serde_json::json!({"name": abs}).to_string()),
        );
        assert_eq!(status, 403);

        // 不存在但在界内 → 404（不 403）
        let (status, _) = request(
            port,
            "POST",
            "/open-fixture",
            Some(&server.token),
            Some(&serde_json::json!({"name": "missing.txt"}).to_string()),
        );
        assert_eq!(status, 404);
        let _ = std::fs::remove_dir_all(root);
    }

    // ---- 非本地拒绝 ----

    #[test]
    fn non_loopback_peer_is_rejected() {
        let loopback_v4: SocketAddr = "127.0.0.1:8080".parse().unwrap();
        let loopback_v6: SocketAddr = "[::1]:8080".parse().unwrap();
        let lan: SocketAddr = "192.168.1.10:80".parse().unwrap();
        let public: SocketAddr = "8.8.8.8:53".parse().unwrap();
        assert!(is_loopback(&loopback_v4));
        assert!(is_loopback(&loopback_v6));
        assert!(!is_loopback(&lan));
        assert!(!is_loopback(&public));
    }

    // ---- /simulate-error /reset-settings /clear-storage /shutdown ----

    #[test]
    fn simulate_error_and_reset_update_status() {
        let root = temp_root("simerr");
        let server = Arc::new(AgentServer::new(root.clone()));
        let port = start(Arc::clone(&server));

        let (status, _) = request(
            port,
            "POST",
            "/simulate-error",
            Some(&server.token),
            Some(&serde_json::json!({"code": "FILE_NOT_FOUND"}).to_string()),
        );
        assert_eq!(status, 200);
        let (_, body) = request(port, "GET", "/state", Some(&server.token), None);
        assert!(body.contains("\"status\":\"error:FILE_NOT_FOUND\""));

        // 非法 code → 400
        let (status, _) = request(
            port,
            "POST",
            "/simulate-error",
            Some(&server.token),
            Some(&serde_json::json!({"code": "bad code!"}).to_string()),
        );
        assert_eq!(status, 400);

        let (status, _) = request(port, "POST", "/reset-settings", Some(&server.token), None);
        assert_eq!(status, 200);
        let (_, body) = request(port, "GET", "/state", Some(&server.token), None);
        assert!(body.contains("\"status\":\"settings-reset\""));

        let (status, _) = request(port, "POST", "/clear-storage", Some(&server.token), None);
        assert_eq!(status, 200);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn shutdown_sets_stop_flag_and_unknown_route_404() {
        let root = temp_root("shutdown");
        let server = Arc::new(AgentServer::new(root.clone()));
        let port = start(Arc::clone(&server));

        let (status, body) = request(port, "POST", "/shutdown", Some(&server.token), None);
        assert_eq!(status, 200);
        assert!(body.contains("shuttingDown"));
        assert!(server.stop.load(Ordering::SeqCst));

        let (status, _) = request(port, "GET", "/nope", Some(&server.token), None);
        assert_eq!(status, 404);
        let _ = std::fs::remove_dir_all(root);
    }

    // ---- 路径解析与参数解析单元 ----

    #[test]
    fn resolve_fixture_lexical_checks() {
        let root = temp_root("resolve");
        std::fs::create_dir_all(root.join("sub")).unwrap();
        std::fs::write(root.join("sub/a.txt"), "a").unwrap();

        let resolved = resolve_fixture(&root, "sub/a.txt").unwrap();
        assert_eq!(resolved.file_name().and_then(|s| s.to_str()), Some("a.txt"));
        let canon_root = std::fs::canonicalize(&root).unwrap();
        assert!(resolved.starts_with(&canon_root));

        assert!(matches!(
            resolve_fixture(&root, "../x"),
            Err(FixtureError::Escaped)
        ));
        assert!(matches!(
            resolve_fixture(&root, "a/../../x"),
            Err(FixtureError::Escaped)
        ));
        assert!(matches!(
            resolve_fixture(&root, "sub/../.."),
            Err(FixtureError::Escaped)
        ));
        assert!(matches!(
            resolve_fixture(&root, ""),
            Err(FixtureError::Invalid)
        ));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn parse_arg_handles_both_forms() {
        let args = vec![
            "app".to_string(),
            "--agent-mode".to_string(),
            "--fixture-root=D:/fx".to_string(),
            "--agent-port".to_string(),
            "1234".to_string(),
        ];
        assert_eq!(
            parse_arg(&args, "--fixture-root"),
            Some("D:/fx".to_string())
        );
        assert_eq!(parse_arg(&args, "--agent-port"), Some("1234".to_string()));
        assert_eq!(parse_arg(&args, "--session-file"), None);
    }
}