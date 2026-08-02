//! featherview-core：览匣 FeatherView 的文本读取与编码检测核心库（纯 Rust，无 Tauri 依赖）。
//!
//! 消费方（架构合同 `docs/architecture/nightly-contract.md` §13）：
//! - `src-tauri`（主应用文档读取）
//! - `crates/windows-preview-handler`（Windows 资源管理器预览窗格）
//!
//! # API 契约（与 Agent C 同名 crate 保持一致，主 Agent 集成时统一）
//!
//! - [`decode_to_utf8`]：编码检测并转码为 UTF-8，返回 `(文本, 检测到的编码名)`；
//!   二进制内容（NUL 启发式）返回 `Err`。
//! - [`read_text_preview`]：读取文本文件预览；超过 `max_bytes` 时只读开头，
//!   并在文本末尾追加截断提示。
//!
//! 编码检测顺序（与主仓库 `src-tauri/src/document` 提取逻辑一致）：
//! BOM（UTF-8 / UTF-16LE / UTF-16BE）→ UTF-16 无 BOM 启发式 → 严格 UTF-8 → chardetng（GBK 等）。

use std::io::Read;
use std::path::Path;

use chardetng::EncodingDetector;

/// 二进制启发式检测时扫描的字节数。
const BINARY_SCAN_BYTES: usize = 8192;

/// 明确按文本阅读的扩展名（即使含少量 NUL 也按文本处理）。
pub const TEXT_EXTENSIONS: &[&str] = &[
    "md", "markdown", "mdown", "txt", "log", "json", "yaml", "yml", "toml", "xml", "ini", "env",
    "js", "ts", "jsx", "tsx", "vue", "css", "html", "htm", "py", "rs", "java", "c", "cpp", "h",
    "hpp", "cs", "go", "sh", "ps1", "bat", "cmd", "csv", "sql", "svg",
];

/// 明确按二进制处理的扩展名。
pub const BINARY_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "webp", "gif", "bmp", "ico", "avif", "pdf", "zip", "gz", "7z", "rar",
    "exe", "dll", "bin", "wasm", "mp3", "mp4", "woff", "woff2", "ttf", "otf", "eot",
];

/// 本库错误类型（内部用；公开 API 按契约以 `String` 返回错误消息）。
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
enum CoreError {
    #[error("找不到文件：{0}")]
    NotFound(String),
    #[error("没有权限读取：{0}")]
    PermissionDenied(String),
    #[error("该路径不是文件：{0}")]
    NotAFile(String),
    #[error("无法识别的编码或二进制文件")]
    UnknownEncoding,
    #[error("I/O 错误：{0}")]
    Io(String),
}

impl From<std::io::Error> for CoreError {
    fn from(e: std::io::Error) -> Self {
        match e.kind() {
            std::io::ErrorKind::NotFound => CoreError::NotFound(e.to_string()),
            std::io::ErrorKind::PermissionDenied => CoreError::PermissionDenied(e.to_string()),
            _ => CoreError::Io(e.to_string()),
        }
    }
}

fn extension_of(path: &str) -> String {
    Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default()
}

/// 判断是否为二进制文件：优先根据扩展名判断，未知扩展名返回 `None`。
pub fn is_binary_by_extension(path: &str) -> Option<bool> {
    let ext = extension_of(path);
    if ext.is_empty() {
        return None;
    }
    if BINARY_EXTENSIONS.contains(&ext.as_str()) {
        Some(true)
    } else if TEXT_EXTENSIONS.contains(&ext.as_str()) {
        Some(false)
    } else {
        None
    }
}

/// 通过扫描字节内容判断二进制（NUL 字节启发式；先排除 UTF-16 文本）。
pub fn is_binary_bytes(bytes: &[u8]) -> bool {
    // 先排除 UTF-16 文本（ASCII 文本的 UTF-16 编码含大量 0x00）
    if try_utf16_no_bom(bytes).is_some() {
        return false;
    }
    let scan_len = bytes.len().min(BINARY_SCAN_BYTES);
    if scan_len == 0 {
        return false;
    }
    let nul_count = bytes[..scan_len].iter().filter(|&&b| b == 0).count();
    // 前 32 字节中一旦出现 NUL 基本可判定二进制（UTF-16 文本除外，已在上方排除）
    let early = bytes[..bytes.len().min(32)].contains(&0);
    (early && nul_count > 0) || (nul_count * 100 / scan_len > 5)
}

/// 检测编码并转码为 UTF-8 字符串。
///
/// 顺序：BOM → 二进制判定（NUL 启发式）→ UTF-16 无 BOM 启发式 → 严格 UTF-8 → chardetng。
/// 返回 `(UTF-8 文本, 检测到的编码名)`；二进制内容返回 `Err`。
pub fn decode_to_utf8(bytes: &[u8]) -> Result<(String, String), String> {
    if bytes.is_empty() {
        return Ok((String::new(), "UTF-8".to_string()));
    }

    // 1. BOM 检测（必须先于二进制判定：UTF-16 BOM 文件含大量 0x00）
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        let (text, _, _) = encoding_rs::UTF_8.decode(&bytes[3..]);
        return Ok((text.into_owned(), "UTF-8-BOM".to_string()));
    }
    if bytes.starts_with(&[0xFF, 0xFE]) {
        let (text, _, _) = encoding_rs::UTF_16LE.decode(&bytes[2..]);
        return Ok((text.into_owned(), "UTF-16LE".to_string()));
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        let (text, _, _) = encoding_rs::UTF_16BE.decode(&bytes[2..]);
        return Ok((text.into_owned(), "UTF-16BE".to_string()));
    }

    // 2. 二进制判定
    if is_binary_bytes(bytes) {
        return Err(CoreError::UnknownEncoding.to_string());
    }

    // 3. UTF-16 无 BOM 启发式（必须先于严格 UTF-8：ASCII 内容的 UTF-16 字节流是合法 UTF-8）
    if let Some((text, endian)) = try_utf16_no_bom(bytes) {
        return Ok((text, endian));
    }

    // 4. 严格 UTF-8
    if let Ok(text) = std::str::from_utf8(bytes) {
        return Ok((text.to_string(), "UTF-8".to_string()));
    }

    // 5. chardetng 检测（针对 GBK 等中文编码）
    let mut detector = EncodingDetector::new();
    let feed_len = bytes.len().min(8192);
    detector.feed(&bytes[..feed_len], true);
    let encoding = detector.guess(None, false);
    let (text, _, _) = encoding.decode(bytes);
    let name = encoding.name().to_string();
    if text.chars().any(|c| c == '\u{FFFD}') && name == "windows-1252" {
        // 解码结果中出现替换符且落入兜底编码，视为识别失败
        return Err(CoreError::UnknownEncoding.to_string());
    }
    Ok((text.into_owned(), name))
}

/// UTF-16 无 BOM 启发式检测：ASCII 文本在 UTF-16 中表现为大量交替 0x00。
fn try_utf16_no_bom(bytes: &[u8]) -> Option<(String, String)> {
    if bytes.len() < 4 || bytes.len() % 2 != 0 {
        return None;
    }
    let scan = bytes.len().min(2048);
    let (even_zero, odd_zero) =
        bytes[..scan]
            .chunks(2)
            .fold((0usize, 0usize), |(even, odd), pair| {
                if pair.len() == 2 {
                    (
                        even + usize::from(pair[0] == 0),
                        odd + usize::from(pair[1] == 0),
                    )
                } else {
                    (even, odd)
                }
            });
    let half = scan / 2;
    // 小端：低字节在偶数位（ASCII 时偶数位非 0、奇数位为 0）
    // 阈值放宽到奇数位 ≥60% 为零、偶数位 ≤20% 为零，以覆盖含中文的 UTF-16 文本
    if odd_zero * 5 >= half * 3 && even_zero * 5 <= half {
        let (text, _, _) = encoding_rs::UTF_16LE.decode(bytes);
        return Some((text.into_owned(), "UTF-16LE".to_string()));
    }
    if even_zero * 5 >= half * 3 && odd_zero * 5 <= half {
        let (text, _, _) = encoding_rs::UTF_16BE.decode(bytes);
        return Some((text.into_owned(), "UTF-16BE".to_string()));
    }
    None
}

/// 读取文本文件预览。
///
/// - 文件超过 `max_bytes` 时只读取开头 `max_bytes` 字节，并在文本末尾追加
///   `[内容过长，仅预览前部]` 提示。
/// - 二进制判定：扩展名优先，其次 NUL 内容扫描；二进制返回 `Err`。
pub fn read_text_preview(path: &str, max_bytes: u64) -> Result<String, String> {
    let meta = std::fs::metadata(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => format!("找不到文件：{path}"),
        std::io::ErrorKind::PermissionDenied => format!("没有权限读取：{path}"),
        _ => format!("读取文件失败：{e}"),
    })?;

    if !meta.is_file() {
        return Err(format!("该路径不是文件：{path}"));
    }

    let truncated = meta.len() > max_bytes;
    let mut file = std::fs::File::open(path)
        .map_err(|e| format!("打开文件失败：{e}"))?;

    // 有上限读取：先探测 max_bytes+1 字节，确认是否截断
    let probe = max_bytes.saturating_add(1).min(usize::MAX as u64) as usize;
    let mut bytes = Vec::with_capacity(probe.min(8 * 1024 * 1024));
    let mut chunk = [0u8; 8192];
    loop {
        let n = file
            .read(&mut chunk)
            .map_err(|e| format!("读取文件失败：{e}"))?;
        if n == 0 {
            break;
        }
        let remaining = max_bytes.saturating_sub(bytes.len() as u64) as usize;
        if remaining == 0 {
            break;
        }
        let take = n.min(remaining);
        bytes.extend_from_slice(&chunk[..take]);
        if take < n {
            break;
        }
        if bytes.len() as u64 >= max_bytes {
            // 已读满 max_bytes；再探一字节确认文件是否更长
            let n2 = file
                .read(&mut chunk)
                .map_err(|e| format!("读取文件失败：{e}"))?;
            if n2 > 0 {
                break; // 还有内容 → 截断
            }
            break;
        }
    }

    // 二进制判定：扩展名优先，其次内容扫描
    let binary = match is_binary_by_extension(path) {
        Some(b) => b,
        None => is_binary_bytes(&bytes),
    };
    if binary {
        return Err("二进制文件，无法作为文本预览".to_string());
    }

    let (text, _encoding) = decode_to_utf8(&bytes)?;
    let mut out = text;
    if truncated {
        out.push_str("\n\n[内容过长，仅预览前部]");
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("featherview-core-test-{name}-{}", std::process::id()));
        p
    }

    fn write_temp(name: &str, bytes: &[u8]) -> String {
        let p = temp_path(name);
        std::fs::write(&p, bytes).unwrap();
        p.to_str().unwrap().to_string()
    }

    fn cleanup(name: &str) {
        let _ = std::fs::remove_file(temp_path(name));
    }

    #[test]
    fn decodes_utf8() {
        let (text, enc) = decode_to_utf8("你好，FeatherView！\n第二行".as_bytes()).unwrap();
        assert!(text.contains("FeatherView"));
        assert_eq!(enc, "UTF-8");
    }

    #[test]
    fn handles_utf8_bom() {
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice("带 BOM 的中文".as_bytes());
        let (text, enc) = decode_to_utf8(&bytes).unwrap();
        assert_eq!(text, "带 BOM 的中文");
        assert_eq!(enc, "UTF-8-BOM");
    }

    #[test]
    fn decodes_utf16le_with_bom() {
        let mut bytes = vec![0xFF, 0xFE];
        bytes.extend(
            "UTF-16 编码测试 hello"
                .encode_utf16()
                .flat_map(|u| u.to_le_bytes()),
        );
        let (text, enc) = decode_to_utf8(&bytes).unwrap();
        assert_eq!(text, "UTF-16 编码测试 hello");
        assert_eq!(enc, "UTF-16LE");
    }

    #[test]
    fn decodes_utf16be_with_bom() {
        let mut bytes = vec![0xFE, 0xFF];
        bytes.extend("大端测试".encode_utf16().flat_map(|u| u.to_be_bytes()));
        let (text, enc) = decode_to_utf8(&bytes).unwrap();
        assert_eq!(text, "大端测试");
        assert_eq!(enc, "UTF-16BE");
    }

    #[test]
    fn decodes_utf16_without_bom_heuristic() {
        let encoded: Vec<u8> = "Hello, FeatherView!"
            .encode_utf16()
            .flat_map(|u| u.to_le_bytes())
            .collect();
        let (text, enc) = decode_to_utf8(&encoded).unwrap();
        assert_eq!(text, "Hello, FeatherView!");
        assert_eq!(enc, "UTF-16LE");
    }

    #[test]
    fn decodes_gbk_chinese() {
        let (encoded, _, _) = encoding_rs::GBK.encode("这是 GBK 编码的中文内容。");
        let (text, _) = decode_to_utf8(&encoded).unwrap();
        assert_eq!(text, "这是 GBK 编码的中文内容。");
    }

    #[test]
    fn empty_bytes_ok() {
        let (text, enc) = decode_to_utf8(&[]).unwrap();
        assert_eq!(text, "");
        assert_eq!(enc, "UTF-8");
    }

    #[test]
    fn rejects_binary_by_content() {
        // PNG 魔数 + NUL 字节
        let png: Vec<u8> = vec![
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0, 1, 2, 3, 0, 0, 0, 0,
        ];
        assert!(is_binary_bytes(&png));
        assert!(decode_to_utf8(&png).is_err());
        assert!(!is_binary_bytes("纯文本内容，没有任何二进制特征".as_bytes()));
    }

    #[test]
    fn binary_by_extension() {
        assert_eq!(is_binary_by_extension("a.png"), Some(true));
        assert_eq!(is_binary_by_extension("a.md"), Some(false));
        assert_eq!(is_binary_by_extension("a.unknown_ext"), None);
    }

    #[test]
    fn preview_reads_utf8_file() {
        let path = write_temp("utf8", "你好，FeatherView！\n第二行".as_bytes());
        let text = read_text_preview(&path, 1024 * 1024).unwrap();
        assert!(text.contains("FeatherView"));
        cleanup("utf8");
    }

    #[test]
    fn preview_truncates_large_file() {
        // 1MB 文件 + 1KB 上限 → 截断提示
        let data = vec![b'a'; 1024 * 1024];
        let path = write_temp("large", &data);
        let text = read_text_preview(&path, 1024).unwrap();
        assert!(text.contains("[内容过长，仅预览前部]"));
        assert!(text.len() < 2000);
        cleanup("large");
    }

    #[test]
    fn preview_rejects_binary_file() {
        let path = write_temp("bin", &[0x89, 0x50, 0x4E, 0x47, 0x00, 0x01, 0x02, 0x03]);
        assert!(read_text_preview(&path, 1024 * 1024).is_err());
        cleanup("bin");
    }

    #[test]
    fn preview_missing_file_errors() {
        let missing = temp_path("missing-file.md");
        let err = read_text_preview(missing.to_str().unwrap(), 1024).unwrap_err();
        assert!(err.contains("找不到文件"));
    }
}