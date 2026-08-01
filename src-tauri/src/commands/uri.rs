//! Android content:// URI 读取命令（夜间架构合同 §9）。
//!
//! 命令（未注册 lib.rs，注册代码见 Agent F 报告 §4）：
//! - `read_uri(uri, offset?, length?)`：读取 URI 指向的文件内容
//! - `copy_uri_to_temp(uri)`：Android 上把 content:// 拷贝到应用缓存（经 Kotlin 桥接）
//! - `uri_temp_info(uri)`：查询 URI 的本地缓存状态
//!
//! 平台策略：
//! - 桌面：支持 `file://` URI（转路径后走文件读取）；`content://` 返回 `URI_UNSUPPORTED`。
//! - Android：Kotlin 桥接（FeatherViewPlugin，scripts/android/FeatherViewPlugin.kt）把
//!   content:// 流式拷贝到 `cacheDir/featherview-uris/<fnv1a64(uri)><ext>`，本模块读取
//!   本地缓存文件（内存映射 + 磁盘扫描重建）；缓存未命中返回 `URI_NOT_CACHED`，
//!   前端引导"导入"流程（copy_uri_to_temp）。
//!
//! 错误序列化独立于 error.rs（避免与其他 Agent 的共享修改冲突）：UriError 手工实现
//! Serialize，输出 `{ title, message, code }`，与 AppError 格式一致
//! （前端 documentService.normalizeError 直接兼容）。

use std::{
    collections::HashMap,
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::Mutex,
};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::document;

/// URI 读取结果（字段与合同 §2 ReadRangeResult 对齐；camelCase 序列化）。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UriReadResult {
    /// 本次实际读取的字节数。
    pub bytes_read: usize,
    /// 下一次读取的起始偏移（字符对齐后的有效解码末尾，保证分块拼接连续）。
    pub next_offset: u64,
    /// 是否已到文件末尾。
    pub eof: bool,
    /// 文本内容（is_binary 为 false 时存在，已转码 UTF-8）。
    pub text: Option<String>,
    /// 检测到的原始编码。
    pub encoding: Option<String>,
    /// 是否二进制文件。
    pub is_binary: bool,
    /// 文件总大小（字节）。
    pub size: u64,
}

/// URI 本地缓存信息。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UriCacheInfo {
    pub cached: bool,
    pub path: Option<String>,
    pub size: Option<u64>,
    pub name: Option<String>,
    pub mime_type: Option<String>,
}

/// Kotlin copyToTemp 命令的返回结构（字段来自 JSObject，camelCase）。
/// 注：仅 Android 分支使用（desktop 构建不引用），允许 dead_code。
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
#[serde(rename_all = "camelCase")]
struct CopyResult {
    path: Option<String>,
    size: Option<u64>,
    name: Option<String>,
    mime_type: Option<String>,
    cache_dir: Option<String>,
    /// Kotlin 侧异常信息（存在即失败）。
    error: Option<String>,
}

/// URI 命令错误：序列化格式与 AppError 一致（title/message/code）。
/// 注：部分变体仅在 Android 分支构造（desktop 构建不引用），允许 dead_code。
#[derive(Debug, thiserror::Error)]
#[allow(dead_code)]
pub enum UriError {
    #[error("该文件尚未导入应用缓存")]
    NotCached(String),
    #[error("当前平台不支持读取该 URI")]
    Unsupported(String),
    #[error("非 UTF-8 编码暂不支持范围读取，请全量读取")]
    RangeEncodingUnsupported,
    #[error("Android 原生桥接未就绪")]
    BridgeUnavailable(String),
    #[error("读取文件失败")]
    Io(String),
    #[error("内部错误")]
    Internal(String),
}

impl UriError {
    pub fn title(&self) -> &'static str {
        match self {
            UriError::NotCached(_) => "文件尚未导入",
            UriError::Unsupported(_) => "不支持的来源",
            UriError::RangeEncodingUnsupported => "暂不支持范围读取",
            UriError::BridgeUnavailable(_) => "Android 原生能力未就绪",
            UriError::Io(_) => "读取失败",
            UriError::Internal(_) => "发生错误",
        }
    }

    pub fn user_message(&self) -> String {
        match self {
            UriError::NotCached(u) => format!(
                "该文件（{u}）尚未导入应用缓存。\n请先在应用内重新选择该文件后重试。"
            ),
            UriError::Unsupported(m) => m.clone(),
            UriError::RangeEncodingUnsupported => {
                "非 UTF-8 编码的文件暂不支持范围读取，请使用全量读取。".to_string()
            }
            UriError::BridgeUnavailable(m) => format!(
                "{m}\n（Android 原生桥接尚未启用，content:// 读取能力暂不可用。）"
            ),
            UriError::Io(m) => m.clone(),
            UriError::Internal(m) => m.clone(),
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            UriError::NotCached(_) => "URI_NOT_CACHED",
            UriError::Unsupported(_) => "URI_UNSUPPORTED",
            UriError::RangeEncodingUnsupported => "URI_RANGE_ENCODING_UNSUPPORTED",
            UriError::BridgeUnavailable(_) => "URI_BRIDGE_UNAVAILABLE",
            UriError::Io(_) => "URI_IO_ERROR",
            UriError::Internal(_) => "URI_INTERNAL",
        }
    }
}

impl Serialize for UriError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut st = s.serialize_struct("UriError", 3)?;
        st.serialize_field("title", self.title())?;
        st.serialize_field("message", &self.user_message())?;
        st.serialize_field("code", self.code())?;
        st.end()
    }
}

impl From<std::io::Error> for UriError {
    fn from(e: std::io::Error) -> Self {
        UriError::Io(e.to_string())
    }
}

/// 进程内 URI → 缓存文件路径映射（重启后经磁盘扫描重建）。
#[derive(Default)]
pub struct UriCacheState(Mutex<HashMap<String, String>>);

impl UriCacheState {
    /// 注：insert/get 仅 Android 分支调用（desktop 构建不引用），允许 dead_code。
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(dead_code)]
    pub fn insert(&self, uri: &str, path: String) {
        self.0.lock().unwrap().insert(uri.to_string(), path);
    }

    #[allow(dead_code)]
    pub fn get(&self, uri: &str) -> Option<String> {
        self.0.lock().unwrap().get(uri).cloned()
    }
}

/// Android 原生桥接句柄：lib.rs 插件 setup 中 register_android_plugin 成功后放入
/// managed state（仅 Android 构建创建与使用；desktop 不引用，随 cfg 编译）。
#[cfg(target_os = "android")]
#[derive(Clone)]
pub struct AndroidBridge(pub tauri::plugin::PluginHandle<tauri::Wry>);

/// Kotlin 侧返回的缓存目录（进程内缓存，首次 copyToTemp/cacheDir 后获得）。
#[cfg(target_os = "android")]
static URI_CACHE_DIR: OnceLock<String> = OnceLock::new();

/// 单次范围读取的最大字节数（与合同 §2 一致）。
const MAX_RANGE_BYTES: u64 = 1024 * 1024;

/// 读取 URI 指向的文件。
///
/// - `file://`（桌面/移动通用）：转本地路径读取。
/// - `content://`（仅 Android）：读取 Kotlin 桥接预拷贝的缓存文件；未缓存返回
///   `URI_NOT_CACHED`（前端先调 copy_uri_to_temp）。
/// - `offset=None` → 全量读取（等同 read_file 语义）；提供 offset/length → 字节范围
///   读取（默认/上限 1MB，UTF-8 字符边界对齐）。
#[tauri::command]
pub async fn read_uri(
    app: AppHandle,
    uri: String,
    offset: Option<u64>,
    length: Option<u32>,
) -> Result<UriReadResult, UriError> {
    if uri.starts_with("file://") {
        let path = file_uri_to_path(&uri)
            .ok_or_else(|| UriError::Unsupported(format!("无法解析 file:// URI：{uri}")))?;
        return read_path_range(&path, offset, length);
    }
    if uri.starts_with("content://") {
        #[cfg(target_os = "android")]
        {
            let path = resolve_cached_path(&app, &uri)?;
            return read_path_range(&path, offset, length);
        }
        #[cfg(not(target_os = "android"))]
        {
            let _ = &app;
            return Err(UriError::Unsupported(
                "content:// 仅在 Android 平台支持。".to_string(),
            ));
        }
    }
    Err(UriError::Unsupported(format!(
        "仅支持 content:// 或 file:// 协议：{uri}"
    )))
}

/// 把 content:// URI 拷贝到应用缓存（Android；经 Kotlin 桥接）。
#[tauri::command]
pub async fn copy_uri_to_temp(app: AppHandle, uri: String) -> Result<UriCacheInfo, UriError> {
    if !uri.starts_with("content://") {
        return Err(UriError::Unsupported(
            "copy_uri_to_temp 仅支持 content:// URI。".to_string(),
        ));
    }
    #[cfg(target_os = "android")]
    {
        let bridge = app.try_state::<AndroidBridge>().ok_or_else(|| {
            UriError::BridgeUnavailable(
                "Kotlin 桥接未注册（FeatherViewPlugin 缺失或注册失败）。".to_string(),
            )
        })?;
        let payload = serde_json::json!({ "uri": uri });
        let res: CopyResult = bridge
            .0
            .run_mobile_plugin_async("copyToTemp", payload)
            .await
            .map_err(|e| UriError::BridgeUnavailable(e.to_string()))?;
        if let Some(err) = res.error {
            return Err(UriError::Io(format!("Android 拷贝失败：{err}")));
        }
        let path = res
            .path
            .ok_or_else(|| UriError::Internal("copyToTemp 未返回 path".to_string()))?;
        if let Some(cache_dir) = res.cache_dir {
            let _ = URI_CACHE_DIR.set(cache_dir);
        }
        if let Some(state) = app.try_state::<UriCacheState>() {
            state.insert(&uri, path.clone());
        }
        Ok(UriCacheInfo {
            cached: true,
            path: Some(path),
            size: res.size,
            name: res.name,
            mime_type: res.mime_type,
        })
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = &app;
        Err(UriError::Unsupported(
            "copy_uri_to_temp 仅在 Android 平台支持。".to_string(),
        ))
    }
}

/// 查询 content:// URI 的本地缓存状态（Android）。
#[tauri::command]
pub async fn uri_temp_info(app: AppHandle, uri: String) -> Result<UriCacheInfo, UriError> {
    if !uri.starts_with("content://") {
        return Err(UriError::Unsupported("仅支持 content:// URI。".to_string()));
    }
    #[cfg(target_os = "android")]
    {
        if let Ok(path) = resolve_cached_path(&app, &uri) {
            let meta = std::fs::metadata(&path).map_err(|e| UriError::Io(e.to_string()))?;
            return Ok(UriCacheInfo {
                cached: true,
                path: Some(path.to_string_lossy().into_owned()),
                size: Some(meta.len()),
                name: None,
                mime_type: None,
            });
        }
        Ok(UriCacheInfo {
            cached: false,
            path: None,
            size: None,
            name: None,
            mime_type: None,
        })
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = &app;
        Err(UriError::Unsupported(
            "uri_temp_info 仅在 Android 平台支持。".to_string(),
        ))
    }
}

/// 按范围读取本地文件。
///
/// - offset=None（或覆盖全文件）→ 全量读取并转码（等同 read_file 语义）。
/// - 范围读取：读入 [offset-4, end+8] 冗余缓冲，UTF-8 严格解码并按字符边界对齐
///   （覆盖 [offset, end) 的最小完整字符集）；`next_offset` 为实际输出末尾。
///   非 UTF-8 内容返回 `URI_RANGE_ENCODING_UNSUPPORTED`（请全量读取）。
fn read_path_range(path: &Path, offset: Option<u64>, length: Option<u32>) -> Result<UriReadResult, UriError> {
    let meta = std::fs::metadata(path)
        .map_err(|e| UriError::Io(format!("{}: {e}", path.display())))?;
    if !meta.is_file() {
        return Err(UriError::Io(format!("{} 不是文件", path.display())));
    }
    let size = meta.len();
    let start = offset.unwrap_or(0).min(size);
    let len = u64::from(length.unwrap_or(MAX_RANGE_BYTES as u32)).min(MAX_RANGE_BYTES);
    let end = start.saturating_add(len).min(size);

    // 全量：与 read_file 语义一致（编码检测覆盖 BOM/UTF-16/GBK 等）
    if start == 0 && end >= size {
        let bytes = std::fs::read(path)
            .map_err(|e| UriError::Io(format!("{}: {e}", path.display())))?;
        let is_binary = document::is_binary_by_extension(&path.to_string_lossy())
            .unwrap_or_else(|| document::is_binary_bytes(&bytes));
        if is_binary {
            return Ok(UriReadResult {
                bytes_read: bytes.len(),
                next_offset: size,
                eof: true,
                text: None,
                encoding: None,
                is_binary: true,
                size,
            });
        }
        let (text, encoding) =
            document::decode_to_utf8(&bytes).map_err(|e| UriError::Io(e.to_string()))?;
        return Ok(UriReadResult {
            bytes_read: bytes.len(),
            next_offset: size,
            eof: true,
            text: Some(text),
            encoding: Some(encoding),
            is_binary: false,
            size,
        });
    }

    // 范围读取：冗余读入 [start-4, end+8]（UTF-8 字符最长 4 字节）
    let read_start = start.saturating_sub(4);
    let read_end = (end + 8).min(size);
    let mut f = File::open(path).map_err(|e| UriError::Io(format!("{}: {e}", path.display())))?;
    f.seek(SeekFrom::Start(read_start))
        .map_err(|e| UriError::Io(e.to_string()))?;
    let mut buf = vec![0u8; (read_end - read_start) as usize];
    let mut read_n = 0usize;
    if !buf.is_empty() {
        read_n = f.read(&mut buf).map_err(|e| UriError::Io(e.to_string()))?;
        buf.truncate(read_n);
    }

    let is_binary = document::is_binary_by_extension(&path.to_string_lossy())
        .unwrap_or_else(|| document::is_binary_bytes(&buf));
    if is_binary {
        return Ok(UriReadResult {
            bytes_read: read_n,
            next_offset: end,
            eof: end >= size,
            text: None,
            encoding: None,
            is_binary: true,
            size,
        });
    }

    // UTF-8 字符对齐：输出覆盖 [start, end) 的最小完整字符集
    let rel_start = (start - read_start) as usize;
    let rel_end = (end - read_start) as usize;
    let out_start = aligned_char_start(&buf, rel_start);
    let out_end = {
        let pos = aligned_char_start(&buf, rel_end);
        if pos < buf.len() {
            (pos + utf8_seq_len(buf[pos])).min(buf.len())
        } else {
            buf.len()
        }
    };
    let slice = &buf[out_start..out_end];
    let text = std::str::from_utf8(slice)
        .map_err(|_| UriError::RangeEncodingUnsupported)?
        .to_string();
    let effective_end = read_start + out_end as u64;
    Ok(UriReadResult {
        bytes_read: read_n,
        next_offset: effective_end.min(size),
        eof: end >= size,
        text: Some(text),
        encoding: Some("UTF-8".to_string()),
        is_binary: false,
        size,
    })
}

/// UTF-8 序列长度（首字节判定；非法 continuation 按 1 处理）。
fn utf8_seq_len(b: u8) -> usize {
    if b < 0x80 {
        1
    } else if b >> 5 == 0b110 {
        2
    } else if b >> 4 == 0b1110 {
        3
    } else if b >> 3 == 0b11110 {
        4
    } else {
        1
    }
}

/// 返回缓冲中包含 byte_offset 的字符起点（跳过头部被截断的孤立 continuation 字节）。
fn aligned_char_start(buf: &[u8], byte_offset: usize) -> usize {
    let mut pos = 0usize;
    // 头部孤立 continuation：前一字符被冗余区起点截断的残留，属于输出范围之前，丢弃
    while pos < buf.len() && (buf[pos] & 0xC0) == 0x80 {
        pos += 1;
    }
    while pos < buf.len() {
        let len = utf8_seq_len(buf[pos]);
        if pos + len > byte_offset {
            return pos;
        }
        pos += len;
    }
    pos
}

/// FNV-1a 64 哈希（与 Kotlin FeatherViewPlugin.cacheFileName 位模式一致）。
/// 注：仅 Android 分支与测试使用（desktop 构建不引用），允许 dead_code。
#[allow(dead_code)]
pub fn cache_file_name(uri: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in uri.as_bytes() {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// 解析 file:// URI 为本地路径（支持 file:///C:/... 与 file:///unix/path；拒绝远端 host）。
fn file_uri_to_path(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    let (host, path) = rest.split_once('/')?;
    if !host.is_empty() && host != "localhost" {
        return None;
    }
    let decoded = percent_decode(path)?;
    Some(PathBuf::from(decoded))
}

/// 简单 percent 解码（UTF-8）。
fn percent_decode(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if i + 2 >= bytes.len() {
                return None;
            }
            let hi = hex_val(bytes[i + 1])?;
            let lo = hex_val(bytes[i + 2])?;
            out.push((hi << 4) | lo);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[cfg(target_os = "android")]
fn resolve_cached_path(app: &AppHandle, uri: &str) -> Result<PathBuf, UriError> {
    // 1. 内存映射
    if let Some(state) = app.try_state::<UriCacheState>() {
        if let Some(p) = state.get(uri) {
            if Path::new(&p).is_file() {
                return Ok(PathBuf::from(p));
            }
        }
    }
    // 2. 磁盘扫描重建（缓存文件名 = <fnv1a64(uri)>[.ext]）
    if let Some(dir) = cached_uris_dir(app) {
        let hash = cache_file_name(uri);
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if name.starts_with(&hash) {
                    let path = entry.path();
                    if let Some(state) = app.try_state::<UriCacheState>() {
                        state.insert(uri, path.to_string_lossy().into_owned());
                    }
                    return Ok(path);
                }
            }
        }
    }
    Err(UriError::NotCached(uri.to_string()))
}

#[cfg(target_os = "android")]
fn cached_uris_dir(app: &AppHandle) -> Option<PathBuf> {
    if let Some(dir) = URI_CACHE_DIR.get() {
        return Some(PathBuf::from(dir.clone()));
    }
    // 首次：经 Kotlin 桥接查询缓存目录（cacheDir 命令，毫秒级 JNI 调用）
    let bridge = app.try_state::<AndroidBridge>()?;
    let res: Result<String, _> = bridge
        .0
        .run_mobile_plugin("cacheDir", serde_json::Value::Null);
    if let Ok(dir) = res {
        let _ = URI_CACHE_DIR.set(dir.clone());
        Some(PathBuf::from(dir))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str, bytes: &[u8]) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("featherview-uri-test-{name}-{}", std::process::id()));
        std::fs::write(&p, bytes).unwrap();
        p
    }

    #[test]
    fn fnv_cache_name_is_stable_and_distinct() {
        assert_eq!(
            cache_file_name("content://a/b"),
            cache_file_name("content://a/b")
        );
        assert_ne!(
            cache_file_name("content://a/b"),
            cache_file_name("content://a/c")
        );
        assert_eq!(cache_file_name("content://a/b").len(), 16);
    }

    #[test]
    fn file_uri_to_path_parses_windows_and_rejects_remote() {
        let p = file_uri_to_path("file:///C:/dir/a%20b.md").unwrap();
        assert_eq!(p, PathBuf::from("C:/dir/a b.md"));
        assert!(file_uri_to_path("file://remote/share/x.md").is_none());
        assert!(file_uri_to_path("file:///plain/path.txt").is_some());
    }

    #[test]
    fn read_full_utf8_file() {
        let p = temp_file("full", "你好，FeatherView！\n第二行".as_bytes());
        let r = read_path_range(&p, None, None).unwrap();
        assert!(!r.is_binary);
        assert_eq!(r.encoding.as_deref(), Some("UTF-8"));
        assert!(r.text.unwrap().contains("FeatherView"));
        assert!(r.eof);
        assert_eq!(r.next_offset, r.size);
        let _ = std::fs::remove_file(p);
    }

    #[test]
    fn read_range_aligns_to_char_boundaries() {
        // "0123456789" (10B) + "你好世界" (12B) + "abcdefghij" (10B) = 32B
        let content = "0123456789你好世界abcdefghij";
        let p = temp_file("range", content.as_bytes());

        // offset=10 len=5：请求 [10,15) 落在"你好"上 → 输出整字符 "你好"
        let r = read_path_range(&p, Some(10), Some(5)).unwrap();
        assert_eq!(r.text.as_deref(), Some("你好"));
        assert_eq!(r.next_offset, 16);
        assert!(!r.eof);

        // offset=6 len=10：请求 [6,16) 覆盖"6..9"与"你好世"（'世'被 end 切中→整字符输出）
        let r = read_path_range(&p, Some(6), Some(10)).unwrap();
        assert_eq!(r.text.as_deref(), Some("6789你好世"));
        assert_eq!(r.next_offset, 19);

        // 末尾：offset=31 命中 'j'（31 是最后一个字节）
        let r = read_path_range(&p, Some(31), Some(100)).unwrap();
        assert_eq!(r.text.as_deref(), Some("j"));
        assert!(r.eof);
        assert_eq!(r.next_offset, 32);

        // 拼接连续性：两块输出拼接 == 全量文本
        let a = read_path_range(&p, Some(10), Some(5)).unwrap();
        let b = read_path_range(&p, Some(a.next_offset), Some(5)).unwrap();
        assert_eq!(a.text.unwrap() + &b.text.unwrap(), "你好世界");
        let _ = std::fs::remove_file(p);
    }

    #[test]
    fn read_range_full_equivalent_when_covering_file() {
        let content = "hello world 你好";
        let p = temp_file("cover", content.as_bytes());
        let full = read_path_range(&p, None, None).unwrap();
        let ranged = read_path_range(&p, Some(0), Some(100)).unwrap();
        assert_eq!(full.text, ranged.text);
        let _ = std::fs::remove_file(p);
    }
}