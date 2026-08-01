//! 文档读取与编码检测核心逻辑。
//!
//! 读取流程：
//! 1. 检查文件是否存在、权限、大小限制
//! 2. 读取字节
//! 3. 判断二进制 / 文本
//! 4. 文本：BOM → UTF-8 → UTF-16 启发式 → chardetng → encoding_rs 转码
//! 5. 二进制：返回 base64，由前端渲染器处理

use std::path::Path;

use base64::Engine;
use chardetng::EncodingDetector;

use crate::error::AppError;

/// 普通文本直接完整读取的默认上限（10MB）。
pub const MAX_TEXT_BYTES: u64 = 10 * 1024 * 1024;
/// 二进制文件（图片等）读取上限（50MB）。
pub const MAX_BINARY_BYTES: u64 = 50 * 1024 * 1024;
/// 二进制启发式检测时扫描的字节数。
const BINARY_SCAN_BYTES: usize = 8192;

/// 明确按文本阅读的扩展名（即使含少量 NUL 也按文本处理）。
const TEXT_EXTENSIONS: &[&str] = &[
    "md", "markdown", "mdown", "txt", "log", "json", "yaml", "yml", "toml", "xml", "ini", "env",
    "js", "ts", "jsx", "tsx", "vue", "css", "html", "htm", "py", "rs", "java", "c", "cpp", "h",
    "hpp", "cs", "go", "sh", "ps1", "bat", "cmd", "csv", "sql", "svg",
];

/// 明确按二进制处理的扩展名。
const BINARY_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "webp", "gif", "bmp", "ico", "avif", "pdf", "zip", "gz", "7z", "rar",
    "exe", "dll", "bin", "wasm", "mp3", "mp4", "woff", "woff2", "ttf", "otf", "eot",
];

/// 统一文件读取结果。
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadFileResult {
    /// 文本内容（is_binary 为 false 时存在，已转为 UTF-8）。
    pub content: Option<String>,
    /// 二进制内容 base64（is_binary 为 true 时存在）。
    pub bytes_base64: Option<String>,
    /// 检测到的原始编码（文本文件）。
    pub encoding: Option<String>,
    pub size: u64,
    pub modified_at: Option<u64>,
    pub is_binary: bool,
}

fn extension_of(path: &str) -> String {
    Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default()
}

/// 判断是否为二进制文件。
///
/// 优先根据扩展名判断；扩展名未知时扫描前若干字节中的 NUL。
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

/// 通过扫描字节内容判断二进制（NUL 字节启发式）。
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
/// 顺序：BOM → 严格 UTF-8 → UTF-16 启发式 → chardetng。
pub fn decode_to_utf8(bytes: &[u8]) -> Result<(String, String), AppError> {
    if bytes.is_empty() {
        return Ok((String::new(), "UTF-8".to_string()));
    }

    // 1. BOM 检测
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

    // 2. UTF-16 无 BOM 启发式（必须先于严格 UTF-8：ASCII 内容的 UTF-16 字节流是合法 UTF-8）
    if let Some((text, endian)) = try_utf16_no_bom(bytes) {
        return Ok((text, endian));
    }

    // 3. 严格 UTF-8
    if let Ok(text) = std::str::from_utf8(bytes) {
        return Ok((text.to_string(), "UTF-8".to_string()));
    }

    // 4. chardetng 检测（针对 GBK 等中文编码）
    let mut detector = EncodingDetector::new();
    let feed_len = bytes.len().min(8192);
    detector.feed(&bytes[..feed_len], true);
    let encoding = detector.guess(None, false);
    let (text, _, _) = encoding.decode(bytes);
    let name = encoding.name().to_string();
    if text.chars().any(|c| c == '\u{FFFD}') && name == "windows-1252" {
        // 解码结果中出现大量替换符且落入兜底编码，视为识别失败
        return Err(AppError::UnknownEncoding);
    }
    Ok((text.into_owned(), name))
}

/// UTF-16 无 BOM 启发式检测：ASCII 文本在 UTF-16 中表现为大量交替 0x00。
fn try_utf16_no_bom(bytes: &[u8]) -> Option<(String, String)> {
    if bytes.len() < 4 || !bytes.len().is_multiple_of(2) {
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

/// 读取文件并返回统一结果。
pub fn read_document(path: &str) -> Result<ReadFileResult, AppError> {
    let meta = std::fs::metadata(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => AppError::NotFound(path.to_string()),
        std::io::ErrorKind::PermissionDenied => AppError::PermissionDenied(path.to_string()),
        _ => AppError::from(e),
    })?;

    if !meta.is_file() {
        return Err(AppError::Io("该路径不是文件。".to_string()));
    }

    let ext_binary = is_binary_by_extension(path);
    let size = meta.len();

    // 大小限制：二进制（图片等）放宽到 50MB，文本 10MB
    let limit = if ext_binary == Some(true) {
        MAX_BINARY_BYTES
    } else {
        MAX_TEXT_BYTES
    };
    if size > limit {
        let limit_mb = limit / (1024 * 1024);
        let kind = if ext_binary == Some(true) {
            "图片"
        } else {
            "文本"
        };
        return Err(AppError::TooLarge(format!(
            "该{kind}文件体积为 {}，超过当前 {limit_mb}MB 的读取上限。\n超大文件的分块读取将在后续版本支持。",
            format_size(size)
        )));
    }

    let bytes = std::fs::read(path)?;

    // 二进制判定：扩展名优先，其次 NUL 扫描
    let is_binary = match ext_binary {
        Some(b) => b,
        None => is_binary_bytes(&bytes),
    };

    let modified_at = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64);

    if is_binary {
        return Ok(ReadFileResult {
            content: None,
            bytes_base64: Some(base64_encode(&bytes)),
            encoding: None,
            size,
            modified_at,
            is_binary: true,
        });
    }

    let (content, encoding) = decode_to_utf8(&bytes)?;

    Ok(ReadFileResult {
        content: Some(content),
        bytes_base64: None,
        encoding: Some(encoding),
        size,
        modified_at,
        is_binary: false,
    })
}

/// 人类可读的文件大小。
pub fn format_size(size: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    if size < KB as u64 {
        format!("{size} B")
    } else if size < MB as u64 {
        format!("{:.1} KB", size as f64 / KB)
    } else if size < GB as u64 {
        format!("{:.1} MB", size as f64 / MB)
    } else {
        format!("{:.2} GB", size as f64 / GB)
    }
}

fn base64_encode(bytes: &[u8]) -> String {
    use base64::engine::general_purpose::STANDARD;
    STANDARD.encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_path(name: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("featherview-test-{name}-{}", std::process::id()));
        p
    }

    fn write_temp(name: &str, bytes: &[u8]) -> String {
        let p = temp_path(name);
        let mut f = std::fs::File::create(&p).unwrap();
        f.write_all(bytes).unwrap();
        p.to_str().unwrap().to_string()
    }

    fn cleanup(name: &str) {
        let _ = std::fs::remove_file(temp_path(name));
    }

    #[test]
    fn reads_utf8_file() {
        let path = write_temp("utf8", "你好，FeatherView！\n第二行".as_bytes());
        let result = read_document(&path).unwrap();
        assert!(!result.is_binary);
        assert!(result.content.unwrap().contains("FeatherView"));
        assert_eq!(result.encoding.as_deref(), Some("UTF-8"));
        cleanup("utf8");
    }

    #[test]
    fn handles_utf8_bom() {
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice("带 BOM 的中文".as_bytes());
        let path = write_temp("bom", &bytes);
        let result = read_document(&path).unwrap();
        let content = result.content.unwrap();
        assert_eq!(content, "带 BOM 的中文");
        assert!(!content.starts_with('\u{FEFF}'));
        cleanup("bom");
    }

    #[test]
    fn reads_utf16le_with_bom() {
        let mut bytes = vec![0xFF, 0xFE];
        let mut encoded = "UTF-16 编码测试 hello"
            .encode_utf16()
            .flat_map(|u| u.to_le_bytes())
            .collect::<Vec<u8>>();
        bytes.append(&mut encoded);
        let path = write_temp("u16le", &bytes);
        let result = read_document(&path).unwrap();
        assert_eq!(result.content.unwrap(), "UTF-16 编码测试 hello");
        assert_eq!(result.encoding.as_deref(), Some("UTF-16LE"));
        cleanup("u16le");
    }

    #[test]
    fn reads_utf16be_with_bom() {
        let mut bytes = vec![0xFE, 0xFF];
        let mut encoded = "大端测试"
            .encode_utf16()
            .flat_map(|u| u.to_be_bytes())
            .collect::<Vec<u8>>();
        bytes.append(&mut encoded);
        let path = write_temp("u16be", &bytes);
        let result = read_document(&path).unwrap();
        assert_eq!(result.content.unwrap(), "大端测试");
        assert_eq!(result.encoding.as_deref(), Some("UTF-16BE"));
        cleanup("u16be");
    }

    #[test]
    fn reads_utf16_without_bom_heuristic() {
        let encoded: Vec<u8> = "Hello, FeatherView!"
            .encode_utf16()
            .flat_map(|u| u.to_le_bytes())
            .collect();
        let (text, enc) = decode_to_utf8(&encoded).unwrap();
        assert_eq!(text, "Hello, FeatherView!");
        assert_eq!(enc, "UTF-16LE");
    }

    #[test]
    fn reads_gbk_chinese() {
        let (encoded, _, _) = encoding_rs::GBK.encode("这是 GBK 编码的中文内容。");
        let path = write_temp("gbk", &encoded);
        let result = read_document(&path).unwrap();
        assert_eq!(result.content.unwrap(), "这是 GBK 编码的中文内容。");
        cleanup("gbk");
    }

    #[test]
    fn errors_on_missing_file() {
        let missing = temp_path("missing-file.md");
        let err = read_document(missing.to_str().unwrap()).unwrap_err();
        assert!(matches!(err, AppError::NotFound(_)));
        assert!(err.user_message().contains("找不到"));
    }

    #[test]
    fn detects_binary_by_content() {
        // PNG 魔数 + NUL 字节
        let png: Vec<u8> = vec![
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0, 1, 2, 3, 0, 0, 0, 0,
        ];
        assert!(is_binary_bytes(&png));
        assert!(!is_binary_bytes(
            "纯文本内容，没有任何二进制特征".as_bytes()
        ));
    }

    #[test]
    fn detects_binary_by_extension() {
        assert_eq!(is_binary_by_extension("a.png"), Some(true));
        assert_eq!(is_binary_by_extension("a.md"), Some(false));
        assert_eq!(is_binary_by_extension("a.unknown_ext"), None);
    }

    #[test]
    fn rejects_oversized_text_file() {
        // 构造超过 10MB 的文本文件
        let chunk = vec![b'a'; 1024 * 1024];
        let mut data = Vec::with_capacity(MAX_TEXT_BYTES as usize + 1);
        for _ in 0..=10 {
            data.extend_from_slice(&chunk);
        }
        let path = write_temp("large", &data);
        let err = read_document(&path).unwrap_err();
        assert!(matches!(err, AppError::TooLarge(_)));
        assert!(err.user_message().contains("上限"));
        cleanup("large");
    }

    #[test]
    fn binary_file_returns_base64() {
        let path = write_temp("bin", &[0x89, 0x50, 0x4E, 0x47, 0x00, 0x01, 0x02, 0x03]);
        let result = read_document(&path).unwrap();
        assert!(result.is_binary);
        assert!(result.content.is_none());
        assert!(result.bytes_base64.is_some());
        cleanup("bin");
    }

    #[test]
    fn formats_file_size() {
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(2048), "2.0 KB");
        assert_eq!(format_size(5 * 1024 * 1024), "5.0 MB");
    }
}
