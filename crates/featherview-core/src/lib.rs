//! FeatherView 核心文本处理库（无 Tauri 依赖）。
//!
//! 从 `src-tauri/src/document/mod.rs` 提取，供 `src-tauri` 与
//! `crates/windows-preview-handler` 复用（夜间合同 §13）。提供：
//!
//! - 编码检测与转码：BOM → 严格 UTF-8 → UTF-16 启发式 → chardetng（`decode_to_utf8`）
//! - 按指定编码解码（`decode_with_encoding`，供分块读取使用）
//! - 二进制判定：扩展名优先，内容 NUL 扫描兜底（`is_binary_by_extension` / `is_binary_bytes`）
//! - 分块读取的字符边界对齐：UTF-8 / UTF-16（含代理对）/ GBK / GB18030（`chunk_bounds`）
//! - 工具：`format_size`、`encode_base64`

use std::path::Path;

use base64::Engine;
use chardetng::EncodingDetector;

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

/// 核心错误类型。
///
/// `decode_to_utf8` 识别失败时返回 [`CoreError::UnknownEncoding`]。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, serde::Serialize)]
pub enum CoreError {
    #[error("无法识别文件编码")]
    UnknownEncoding,
    #[error("读取失败：{0}")]
    Io(String),
}

impl From<std::io::Error> for CoreError {
    fn from(e: std::io::Error) -> Self {
        CoreError::Io(e.to_string())
    }
}

fn extension_of(path: &str) -> String {
    Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default()
}

/// 判断是否为二进制文件（扩展名优先）。
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
/// 返回 `(UTF-8 文本, 编码名)`；编码名取值：`UTF-8` / `UTF-8-BOM` / `UTF-16LE` /
/// `UTF-16BE` / `GBK` / `gb18030` / `windows-1252` 等（chardetng 结果）。
pub fn decode_to_utf8(bytes: &[u8]) -> Result<(String, String), CoreError> {
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
        return Err(CoreError::UnknownEncoding);
    }
    Ok((text.into_owned(), name))
}

/// 按指定编码解码为 UTF-8（供分块读取使用，不再重新检测编码）。
///
/// 对 UTF-8 / UTF-16 族手动剥离匹配的 BOM。编码名大小写不敏感。
pub fn decode_with_encoding(bytes: &[u8], encoding: &str) -> Result<String, CoreError> {
    let (enc, bom_len) = match encoding.to_ascii_lowercase().as_str() {
        "utf-8" | "utf-8-bom" => (
            encoding_rs::UTF_8,
            if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
                3
            } else {
                0
            },
        ),
        "utf-16le" => (
            encoding_rs::UTF_16LE,
            if bytes.starts_with(&[0xFF, 0xFE]) {
                2
            } else {
                0
            },
        ),
        "utf-16be" => (
            encoding_rs::UTF_16BE,
            if bytes.starts_with(&[0xFE, 0xFF]) {
                2
            } else {
                0
            },
        ),
        name => (
            encoding_rs::Encoding::for_label(name.as_bytes()).ok_or(CoreError::UnknownEncoding)?,
            0,
        ),
    };
    let (text, _, _) = enc.decode(&bytes[bom_len..]);
    Ok(text.into_owned())
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

/// 计算分块的有效字节范围（相对 `bytes` 的 `[start, end)`），保证字符完整。
///
/// - `bytes`：实际读取的原始字节（已含前后冗余）
/// - `offset`：请求偏移相对 `bytes` 起点的位置
///
/// 返回的 `start` 是完整字符边界（请求偏移落在字符中间时回退到该字符起点，
/// 因此 `start <= offset`）；`end` 是最后一个完整字符的结束边界。
pub fn chunk_bounds(bytes: &[u8], offset: usize, encoding: &str) -> (usize, usize) {
    match encoding.to_ascii_lowercase().as_str() {
        "utf-8" | "utf-8-bom" => utf8_chunk_bounds(bytes, offset),
        "utf-16le" => utf16_chunk_bounds(bytes, offset, false),
        "utf-16be" => utf16_chunk_bounds(bytes, offset, true),
        "gbk" | "gb18030" => gbk_chunk_bounds(bytes, offset),
        // 单字节编码（windows-1252、ISO-8859-* 等）：无字符边界问题
        _ => (offset.min(bytes.len()), bytes.len()),
    }
}

/// UTF-8 分块边界：从缓冲头扫描字符，头部跳过半个字符（续字节开头），
/// 尾部丢弃不完整字符。
pub fn utf8_chunk_bounds(bytes: &[u8], offset: usize) -> (usize, usize) {
    let len = bytes.len();
    if len == 0 {
        return (0, 0);
    }
    let offset = offset.min(len);
    // 头部：跳过开头的续字节（半个字符，起点在缓冲外）
    let mut start = 0usize;
    let mut i = 0usize;
    while i < len && is_utf8_continuation(bytes[i]) {
        i += 1;
        start = i;
    }
    // 扫描：定位包含 offset 的字符起点
    while i < len {
        match utf8_char_len_at(bytes, i) {
            Some(clen) => {
                let next = i + clen;
                if i <= offset && offset < next {
                    start = i;
                    break;
                }
                if offset == next {
                    start = next;
                    break;
                }
                i = next;
            }
            None => break,
        }
    }
    // 尾部：保留完整字符
    let mut end = start;
    let mut j = start;
    while j < len {
        match utf8_char_len_at(bytes, j) {
            Some(clen) => {
                j += clen;
                end = j;
            }
            None => break,
        }
    }
    (start, end)
}

/// UTF-16 分块边界：2 字节单元对齐 + 代理对完整（高低代理跨块合并）。
pub fn utf16_chunk_bounds(bytes: &[u8], offset: usize, big_endian: bool) -> (usize, usize) {
    let len = bytes.len() & !1;
    if len == 0 {
        return (0, 0);
    }
    let offset = (offset.min(len)) & !1;
    // 头部：跳过开头的低代理（半个代理对）
    let mut start = 0usize;
    let mut i = 0usize;
    if is_low_surrogate(read_u16(bytes, 0, big_endian)) {
        i = 2;
        start = 2;
    }
    while i < len {
        let u = read_u16(bytes, i, big_endian);
        let clen = if is_high_surrogate(u)
            && i + 4 <= len
            && is_low_surrogate(read_u16(bytes, i + 2, big_endian))
        {
            4
        } else {
            2
        };
        let next = i + clen;
        if i <= offset && offset < next {
            start = i;
            break;
        }
        if offset == next {
            start = next;
            break;
        }
        i = next;
    }
    // 尾部：最后一个单元若是高代理（缺低代理）则丢弃，等下一块合并
    let mut end = len;
    if end >= 2 && is_high_surrogate(read_u16(bytes, end - 2, big_endian)) {
        end -= 2;
    }
    (start, end.max(start))
}

/// GBK / GB18030 分块边界：双字节（lead+trail）与四字节（lead+digit+lead+digit）
/// 字符序列扫描，头部跳过半个字符，尾部丢弃不完整尾字节（合同 §2 冗余策略）。
pub fn gbk_chunk_bounds(bytes: &[u8], offset: usize) -> (usize, usize) {
    let len = bytes.len();
    if len == 0 {
        return (0, 0);
    }
    let offset = offset.min(len);
    // 头部：跳过开头的半个字符（trail / digit 开头）
    let mut start = 0usize;
    let mut i = 0usize;
    while i < len && !is_gbk_char_start(bytes[i]) {
        i += 1;
        start = i;
    }
    while i < len {
        match gbk_char_len_at(bytes, i) {
            Some(clen) => {
                let next = i + clen;
                if i <= offset && offset < next {
                    start = i;
                    break;
                }
                if offset == next {
                    start = next;
                    break;
                }
                i = next;
            }
            None => break,
        }
    }
    // 尾部：保留完整字符
    let mut end = start;
    let mut j = start;
    while j < len {
        match gbk_char_len_at(bytes, j) {
            Some(clen) => {
                j += clen;
                end = j;
            }
            None => break,
        }
    }
    (start, end)
}

fn is_utf8_continuation(b: u8) -> bool {
    b & 0xC0 == 0x80
}

/// `bytes[i]` 起始的 UTF-8 字符长度；不完整或非法起始字节返回 `None`。
fn utf8_char_len_at(bytes: &[u8], i: usize) -> Option<usize> {
    let b = bytes[i];
    let len = match b {
        0x00..=0x7F => 1,
        0xC2..=0xDF => 2,
        0xE0..=0xEF => 3,
        0xF0..=0xF4 => 4,
        _ => return None,
    };
    if i + len <= bytes.len() && bytes[i + 1..i + len].iter().all(|&x| is_utf8_continuation(x)) {
        Some(len)
    } else {
        None
    }
}

fn read_u16(bytes: &[u8], i: usize, big_endian: bool) -> u16 {
    let hi = u16::from(bytes[i]);
    let lo = u16::from(bytes[i + 1]);
    if big_endian {
        (hi << 8) | lo
    } else {
        (lo << 8) | hi
    }
}

fn is_high_surrogate(u: u16) -> bool {
    (0xD800..=0xDBFF).contains(&u)
}

fn is_low_surrogate(u: u16) -> bool {
    (0xDC00..=0xDFFF).contains(&u)
}

fn is_gbk_lead(b: u8) -> bool {
    (0x81..=0xFE).contains(&b)
}

fn is_gbk_trail(b: u8) -> bool {
    (0x40..=0x7E).contains(&b) || (0x80..=0xFE).contains(&b)
}

fn is_gbk_digit(b: u8) -> bool {
    (0x30..=0x39).contains(&b)
}

/// GBK/GB18030 字符起点：ASCII（含 0x40-0x7E 歧义字节）或 lead 字节。
fn is_gbk_char_start(b: u8) -> bool {
    b < 0x80 || is_gbk_lead(b)
}

/// `bytes[i]` 起始的 GBK/GB18030 字符长度；不完整或非起点返回 `None`。
fn gbk_char_len_at(bytes: &[u8], i: usize) -> Option<usize> {
    let b = bytes[i];
    if b < 0x80 {
        return Some(1);
    }
    if is_gbk_lead(b) {
        if i + 1 < bytes.len() && is_gbk_trail(bytes[i + 1]) {
            return Some(2);
        }
        if i + 3 < bytes.len()
            && is_gbk_digit(bytes[i + 1])
            && is_gbk_lead(bytes[i + 2])
            && is_gbk_digit(bytes[i + 3])
        {
            return Some(4);
        }
        return None;
    }
    None
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

/// Base64 编码（标准表）。
pub fn encode_base64(bytes: &[u8]) -> String {
    use base64::engine::general_purpose::STANDARD;
    STANDARD.encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_utf8_bom() {
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice("带 BOM 的中文".as_bytes());
        let (text, enc) = decode_to_utf8(&bytes).unwrap();
        assert_eq!(text, "带 BOM 的中文");
        assert_eq!(enc, "UTF-8-BOM");
    }

    #[test]
    fn decodes_utf16le_with_bom() {
        let mut bytes = vec![0xFF, 0xFE];
        let mut body: Vec<u8> = "UTF-16 编码测试 hello"
            .encode_utf16()
            .flat_map(|u| u.to_le_bytes())
            .collect();
        bytes.append(&mut body);
        let (text, enc) = decode_to_utf8(&bytes).unwrap();
        assert_eq!(text, "UTF-16 编码测试 hello");
        assert_eq!(enc, "UTF-16LE");
    }

    #[test]
    fn decodes_utf16le_without_bom_by_heuristic() {
        let bytes: Vec<u8> = "Hello, FeatherView!"
            .encode_utf16()
            .flat_map(|u| u.to_le_bytes())
            .collect();
        let (text, enc) = decode_to_utf8(&bytes).unwrap();
        assert_eq!(text, "Hello, FeatherView!");
        assert_eq!(enc, "UTF-16LE");
    }

    #[test]
    fn decodes_strict_utf8() {
        let (text, enc) = decode_to_utf8("你好，FeatherView！".as_bytes()).unwrap();
        assert_eq!(text, "你好，FeatherView！");
        assert_eq!(enc, "UTF-8");
    }

    #[test]
    fn decodes_empty_input() {
        let (text, enc) = decode_to_utf8(b"").unwrap();
        assert_eq!(text, "");
        assert_eq!(enc, "UTF-8");
    }

    #[test]
    fn decodes_gbk_chinese() {
        let (encoded, _, _) = encoding_rs::GBK.encode("这是 GBK 编码的中文内容。");
        let (text, enc) = decode_to_utf8(&encoded).unwrap();
        assert_eq!(text, "这是 GBK 编码的中文内容。");
        assert!(enc == "GBK" || enc == "gb18030", "unexpected encoding: {enc}");
    }

    #[test]
    fn decode_with_encoding_strips_bom() {
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice("你好".as_bytes());
        assert_eq!(decode_with_encoding(&bytes, "UTF-8-BOM").unwrap(), "你好");
        assert_eq!(decode_with_encoding(&bytes, "utf-8").unwrap(), "你好");
        // 无 BOM 输入不受影响
        assert_eq!(decode_with_encoding("你好".as_bytes(), "UTF-8").unwrap(), "你好");
    }

    #[test]
    fn decode_with_encoding_utf16le() {
        let mut bytes = vec![0xFF, 0xFE];
        bytes.extend("中文 UTF-16".encode_utf16().flat_map(|u| u.to_le_bytes()));
        assert_eq!(decode_with_encoding(&bytes, "UTF-16LE").unwrap(), "中文 UTF-16");
        // 无 BOM 的中间块（偶数长度）
        let body: Vec<u8> = "中间块 abc".encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
        assert_eq!(decode_with_encoding(&body, "UTF-16LE").unwrap(), "中间块 abc");
    }

    #[test]
    fn decode_with_encoding_gbk() {
        let (encoded, _, _) = encoding_rs::GBK.encode("GBK 内容 abc");
        assert_eq!(decode_with_encoding(&encoded, "GBK").unwrap(), "GBK 内容 abc");
    }

    #[test]
    fn decode_with_encoding_unknown_label() {
        assert!(matches!(
            decode_with_encoding(b"x", "not-a-real-encoding"),
            Err(CoreError::UnknownEncoding)
        ));
    }

    #[test]
    fn utf8_chunk_bounds_handles_multi_byte_chars() {
        let bytes = "你好世界".as_bytes(); // 4 个 3 字节字符，共 12 字节
        assert_eq!(bytes.len(), 12);
        assert_eq!(utf8_chunk_bounds(bytes, 0), (0, 12));
        assert_eq!(utf8_chunk_bounds(bytes, 1), (0, 12)); // 字符中间 → 回退到起点
        assert_eq!(utf8_chunk_bounds(bytes, 3), (3, 12)); // 边界
        assert_eq!(utf8_chunk_bounds(bytes, 5), (3, 12)); // 字符中间 → 回退到起点
        assert_eq!(utf8_chunk_bounds(bytes, 12), (12, 12)); // 末尾
        // 尾部不完整字符被丢弃
        assert_eq!(utf8_chunk_bounds(&bytes[..11], 0), (0, 9));
    }

    #[test]
    fn utf16_chunk_bounds_keeps_surrogate_pairs() {
        // "A🎉B" UTF-16LE：0041 D83C DF89 0042
        let bytes = [0x41u8, 0x00, 0x3C, 0xD8, 0x89, 0xDF, 0x42, 0x00];
        assert_eq!(utf16_chunk_bounds(&bytes, 0, false), (0, 8));
        assert_eq!(utf16_chunk_bounds(&bytes, 2, false), (2, 8));
        assert_eq!(utf16_chunk_bounds(&bytes, 4, false), (2, 8)); // 代理对中间 → 回退到高代理
        assert_eq!(utf16_chunk_bounds(&bytes, 6, false), (6, 8));
        // 尾部分割：末单元是高代理 → 丢弃，等下一块合并
        let truncated = [0x41u8, 0x00, 0x3C, 0xD8];
        assert_eq!(utf16_chunk_bounds(&truncated, 0, false), (0, 2));
        // 大端
        let be = [0x00u8, 0x41, 0x00, 0x42];
        assert_eq!(utf16_chunk_bounds(&be, 2, true), (2, 4));
    }

    #[test]
    fn gbk_chunk_bounds_handles_double_byte_chars() {
        // "你好" GBK：你=D6 E0，好=BA C3
        let bytes = [0xD6u8, 0xE0, 0xBA, 0xC3];
        assert_eq!(gbk_chunk_bounds(&bytes, 0), (0, 4));
        assert_eq!(gbk_chunk_bounds(&bytes, 1), (0, 4)); // 字符中间 → 回退到起点
        assert_eq!(gbk_chunk_bounds(&bytes, 2), (2, 4));
        assert_eq!(gbk_chunk_bounds(&bytes, 3), (2, 4));
        // 尾部孤立 lead 被丢弃
        let truncated = [0xD6u8, 0xE0, 0xBA];
        assert_eq!(gbk_chunk_bounds(&truncated, 0), (0, 2));
    }

    #[test]
    fn gb18030_chunk_bounds_handles_four_byte_chars() {
        // U+20000（𠀀）GB18030 四字节：95 32 82 36，后跟 ASCII 'A'
        let bytes = [0x95u8, 0x32, 0x82, 0x36, 0x41];
        assert_eq!(gbk_chunk_bounds(&bytes, 0), (0, 5));
        assert_eq!(gbk_chunk_bounds(&bytes, 2), (0, 5)); // 四字节字符中间 → 回退
        assert_eq!(gbk_chunk_bounds(&bytes, 4), (4, 5)); // 'A'
    }

    #[test]
    fn chunk_bounds_dispatches_by_encoding() {
        let bytes = "你好".as_bytes();
        assert_eq!(chunk_bounds(bytes, 1, "UTF-8"), (0, 6));
        assert_eq!(chunk_bounds(bytes, 1, "utf-8-bom"), (0, 6));
        let gbk = [0xD6u8, 0xE0];
        assert_eq!(chunk_bounds(&gbk, 1, "GBK"), (0, 2));
        assert_eq!(chunk_bounds(&gbk, 1, "gb18030"), (0, 2));
        // 单字节编码无边界问题
        let ascii = b"hello";
        assert_eq!(chunk_bounds(ascii, 2, "windows-1252"), (2, 5));
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
    fn formats_file_size() {
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(2048), "2.0 KB");
        assert_eq!(format_size(5 * 1024 * 1024), "5.0 MB");
    }

    #[test]
    fn encodes_base64() {
        assert_eq!(encode_base64(b"hello"), "aGVsbG8=");
    }
}