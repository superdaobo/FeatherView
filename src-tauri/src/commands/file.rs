//! 文件读取相关命令。
//!
//! 大文件范围读取会话（夜间合同 §2/§3/§4）：
//! - `open_read_session`：打开会话（编码一次性检测，前 8KB；返回 LargeFileInfo）
//! - `read_range`：分块读取（UTF-8/UTF-16 字符边界对齐、GBK/GB18030 冗余策略）
//! - `close_read_session`：关闭会话（幂等）
//!
//! 会话表上限 32，超限淘汰最旧（合同 §4）。每次 `read_range` 校验 mtime / size，
//! 文件变化返回 `FILE_CHANGED` 并自动关闭会话（合同 §2）。
//!
//! 注意：分块边界与按编码解码的辅助函数当前内联在本文件（src-tauri 尚未依赖
//! `featherview-core` crate，夜间合同 §13）；主 Agent 切换依赖后可将
//! `chunk_bounds` / `decode_with_encoding` 替换为 `featherview_core` 的对应函数。

use crate::document::{self, ReadFileResult};
use crate::error::AppError;
use std::collections::HashMap;
use std::sync::LazyLock;
use std::io::{Read, Seek, SeekFrom};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Instant;

/// 会话表上限（合同 §4：超限淘汰最旧）。
const MAX_SESSIONS: usize = 32;
/// 单次 read_range 读取上限（合同 §2）。
const MAX_RANGE_BYTES: u64 = 1024 * 1024;
/// 给前端的建议分块大小（合同 §3，默认 256KB）。
const SUGGESTED_CHUNK_BYTES: u64 = 256 * 1024;
/// 会话级编码检测窗口（前 8KB，合同 §2）。
const ENCODING_SNIFF_BYTES: usize = 8192;

/// 范围读取会话。
#[derive(Debug)]
struct ReadSession {
    path: String,
    file_len: u64,
    mtime_ms: Option<u64>,
    encoding: String,
    is_text: bool,
    last_used: Instant,
}

/// 会话快照（锁内复制，IO 在锁外执行）。
#[derive(Debug, Clone)]
struct SessionSnapshot {
    path: String,
    file_len: u64,
    mtime_ms: Option<u64>,
    encoding: String,
}

/// 活跃会话表（进程级）。
static SESSIONS: LazyLock<Mutex<HashMap<String, ReadSession>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
/// sessionId 自增序列。
static SESSION_SEQ: AtomicU64 = AtomicU64::new(0);

fn lock_sessions() -> std::sync::MutexGuard<'static, HashMap<String, ReadSession>> {
    SESSIONS.lock().unwrap_or_else(|e| e.into_inner())
}

fn new_session_id() -> String {
    let seq = SESSION_SEQ.fetch_add(1, Ordering::Relaxed);
    format!("fs-{}-{seq}", std::process::id())
}

/// 统一文件读取命令。
///
/// 前端所有文件读取都经由该命令，返回文本内容或二进制 base64。
#[tauri::command]
pub fn read_file(path: String) -> Result<ReadFileResult, AppError> {
    document::read_document(&path).inspect_err(|e| {
        // 日志中保留详细错误，前端只展示用户可读信息
        eprintln!("[FeatherView] read_file failed: {}", e.log_detail());
    })
}

/// 检查文件是否存在。
#[tauri::command]
pub fn file_exists(path: String) -> bool {
    std::path::Path::new(&path).is_file()
}

/// 读取文件基本信息（不读内容）。
#[tauri::command]
pub fn file_metadata(path: String) -> Result<FileMeta, AppError> {
    let meta = std::fs::metadata(&path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => AppError::NotFound(path.clone()),
        std::io::ErrorKind::PermissionDenied => AppError::PermissionDenied(path.clone()),
        _ => AppError::from(e),
    })?;
    let modified_at = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64);
    Ok(FileMeta {
        size: meta.len(),
        modified_at,
        is_file: meta.is_file(),
    })
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileMeta {
    pub size: u64,
    pub modified_at: Option<u64>,
    pub is_file: bool,
}

/// 打开大文件范围读取会话（合同 §2/§3）。
///
/// 会话级一次性完成二进制判定与编码检测（前 8KB）；返回 `LargeFileInfo`
/// （含 sessionId、编码、建议分块大小），由前端决定是否走虚拟滚动路径。
#[tauri::command]
pub fn open_read_session(path: String) -> Result<LargeFileInfo, AppError> {
    let meta = std::fs::metadata(&path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => AppError::NotFound(path.clone()),
        std::io::ErrorKind::PermissionDenied => AppError::PermissionDenied(path.clone()),
        _ => AppError::from(e),
    })?;
    if !meta.is_file() {
        return Err(AppError::Io("该路径不是文件。".to_string()));
    }
    let file_len = meta.len();
    let mtime_ms = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64);

    // 会话级二进制 / 编码检测（前 8KB 一次性，合同 §2）
    let (is_text, encoding) = detect_text_encoding(&path)?;

    let session_id = new_session_id();
    {
        let mut sessions = lock_sessions();
        // 同一路径的旧会话直接替换，避免重复
        sessions.retain(|_, s| s.path != path);
        // 超限淘汰最旧（合同 §4：上限 32）
        if sessions.len() >= MAX_SESSIONS {
            if let Some(oldest) = sessions
                .iter()
                .min_by_key(|(_, s)| s.last_used)
                .map(|(k, _)| k.clone())
            {
                sessions.remove(&oldest);
            }
        }
        sessions.insert(
            session_id.clone(),
            ReadSession {
                path: path.clone(),
                file_len,
                mtime_ms,
                encoding: encoding.clone(),
                is_text,
                last_used: Instant::now(),
            },
        );
    }

    Ok(LargeFileInfo {
        session_id,
        size: file_len,
        mtime: mtime_ms.unwrap_or(0),
        encoding,
        is_text,
        suggested_chunk_bytes: SUGGESTED_CHUNK_BYTES,
    })
}

/// 读取指定范围（合同 §2）。
///
/// 每次校验文件 mtime / size：变化返回 `FILE_CHANGED` 并自动关闭会话。
/// 单次 `length` 上限 1MB；返回文本已按会话编码转码为 UTF-8，字符边界完整。
#[tauri::command]
pub fn read_range(
    session_id: String,
    offset: u64,
    length: u32,
) -> Result<ReadRangeResult, AppError> {
    // 1. 会话快照（锁内短临界区，无 IO）
    let snapshot = {
        let sessions = lock_sessions();
        let sess = match sessions.get(&session_id) {
            Some(s) => s,
            None => return Err(AppError::SessionNotFound),
        };
        if !sess.is_text {
            return Err(AppError::BinaryFile);
        }
        SessionSnapshot {
            path: sess.path.clone(),
            file_len: sess.file_len,
            mtime_ms: sess.mtime_ms,
            encoding: sess.encoding.clone(),
        }
    };

    // 2. 校验文件未被修改（合同 §2：变化返回 FILE_CHANGED 并移除会话）
    if let Err(err) = verify_unchanged(&snapshot) {
        if let Ok(mut sessions) = SESSIONS.lock() {
            sessions.remove(&session_id);
        }
        return Err(err);
    }

    // 3. 越界 / 空请求
    if offset >= snapshot.file_len {
        return Ok(ReadRangeResult {
            bytes_read: 0,
            next_offset: snapshot.file_len,
            eof: true,
            text: String::new(),
            encoding: snapshot.encoding,
            session_modified_at: snapshot.mtime_ms,
        });
    }
    if length == 0 {
        return Ok(ReadRangeResult {
            bytes_read: 0,
            next_offset: offset,
            eof: false,
            text: String::new(),
            encoding: snapshot.encoding,
            session_modified_at: snapshot.mtime_ms,
        });
    }

    // 4. 计算带冗余的读取范围并执行 IO（锁外）
    let length = u64::from(length.min(MAX_RANGE_BYTES as u32));
    let (read_start, read_end) =
        compute_read_range(&snapshot.encoding, offset, length, snapshot.file_len);
    let result = read_and_decode(&snapshot, offset, read_start, read_end);

    // 5. 刷新最近使用时间（会话可能已被淘汰 / 关闭，允许失败）
    if let Ok(mut sessions) = SESSIONS.lock() {
        if let Some(sess) = sessions.get_mut(&session_id) {
            sess.last_used = Instant::now();
        }
    }

    result
}

/// 关闭读取会话（幂等：会话不存在也返回成功，合同 §4）。
#[tauri::command]
pub fn close_read_session(session_id: String) -> Result<(), AppError> {
    let mut sessions = lock_sessions();
    sessions.remove(&session_id);
    Ok(())
}

/// 会话级二进制 / 编码检测（前 8KB 一次性，合同 §2）。
///
/// 返回 `(is_text, encoding)`；二进制时 encoding 为空字符串。
fn detect_text_encoding(path: &str) -> Result<(bool, String), AppError> {
    let ext_binary = document::is_binary_by_extension(path);
    let head = read_head(path, ENCODING_SNIFF_BYTES)?;
    // BOM 明确指示文本编码：UTF-8 / UTF-16LE / UTF-16BE 一律按文本处理
    let has_text_bom = head.starts_with(&[0xEF, 0xBB, 0xBF])
        || head.starts_with(&[0xFF, 0xFE])
        || head.starts_with(&[0xFE, 0xFF]);
    let is_text = match ext_binary {
        Some(binary) => !binary,
        None => has_text_bom || !document::is_binary_bytes(&head),
    };
    if !is_text {
        return Ok((false, String::new()));
    }
    let encoding = document::decode_to_utf8(&head)
        .map(|(_, enc)| enc)
        .unwrap_or_else(|_| "UTF-8".to_string());
    Ok((true, encoding))
}

/// 读取文件头部最多 `len` 字节。
fn read_head(path: &str, len: usize) -> Result<Vec<u8>, AppError> {
    let file = std::fs::File::open(path)?;
    let mut buf = Vec::with_capacity(len);
    file.take(len as u64).read_to_end(&mut buf)?;
    Ok(buf)
}

/// 校验文件未被修改；变化返回 `FILE_CHANGED`（会话由调用方移除）。
///
/// mtime 均可得时比较 mtime；mtime 不可得（退化平台）时比较 size；
/// 双校验：任一不同即判定已变化（覆盖同毫秒内替换的极端情况）。
fn verify_unchanged(snapshot: &SessionSnapshot) -> Result<(), AppError> {
    let meta = match std::fs::metadata(&snapshot.path) {
        Ok(m) => m,
        Err(e) => {
            return Err(match e.kind() {
                std::io::ErrorKind::NotFound => AppError::NotFound(snapshot.path.clone()),
                std::io::ErrorKind::PermissionDenied => {
                    AppError::PermissionDenied(snapshot.path.clone())
                }
                _ => AppError::from(e),
            });
        }
    };
    let now_mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64);
    let mtime_changed = match (snapshot.mtime_ms, now_mtime) {
        (Some(a), Some(b)) => a != b,
        _ => false,
    };
    let size_changed = meta.len() != snapshot.file_len;
    if mtime_changed || size_changed {
        return Err(AppError::FileChanged(snapshot.path.clone()));
    }
    Ok(())
}

/// 计算实际读取范围：按编码扩展冗余（合同 §2）：
/// - UTF-8：向前 3 / 向后 4 字节冗余
/// - UTF-16：偶数对齐，前后各 4 字节冗余（2 个单元，覆盖代理对）
/// - GBK/GB18030：前后各 4 字节冗余，配合 `chunk_bounds` 丢弃不完整尾字节
/// - 单字节编码：无冗余
fn compute_read_range(encoding: &str, offset: u64, length: u64, file_len: u64) -> (u64, u64) {
    let enc = encoding.to_ascii_lowercase();
    let (start, end) = match enc.as_str() {
        "utf-8" | "utf-8-bom" => (offset.saturating_sub(3), (offset + length + 4).min(file_len)),
        "utf-16le" | "utf-16be" => (
            offset.saturating_sub(4) & !1,
            ((offset + length + 4).min(file_len)) & !1,
        ),
        "gbk" | "gb18030" => (offset.saturating_sub(4), (offset + length + 4).min(file_len)),
        _ => (offset, (offset + length).min(file_len)),
    };
    (start, end.max(start))
}

/// 读取 [read_start, read_end) 并解码为 UTF-8 文本（锁外 IO）。
fn read_and_decode(
    snapshot: &SessionSnapshot,
    offset: u64,
    read_start: u64,
    read_end: u64,
) -> Result<ReadRangeResult, AppError> {
    let mut file = match std::fs::File::open(&snapshot.path) {
        Ok(f) => f,
        Err(e) => {
            return Err(match e.kind() {
                std::io::ErrorKind::NotFound => AppError::NotFound(snapshot.path.clone()),
                std::io::ErrorKind::PermissionDenied => {
                    AppError::PermissionDenied(snapshot.path.clone())
                }
                _ => AppError::from(e),
            });
        }
    };
    file.seek(SeekFrom::Start(read_start))?;
    let mut raw = vec![0u8; (read_end - read_start) as usize];
    let mut filled = 0usize;
    while filled < raw.len() {
        let n = file.read(&mut raw[filled..])?;
        if n == 0 {
            break;
        }
        filled += n;
    }
    raw.truncate(filled);

    let offset_rel = (offset - read_start) as usize;
    let (start, end) = chunk_bounds(&raw, offset_rel, &snapshot.encoding);
    let text = if start < end {
        decode_with_encoding(&raw[start..end], &snapshot.encoding)?
    } else {
        String::new()
    };
    let next_offset = read_start + end as u64;
    // 无进展（请求范围内无完整字符可解码）时视为 EOF，防止前端死循环
    let eof = next_offset >= snapshot.file_len || (start == end && next_offset == offset);
    Ok(ReadRangeResult {
        bytes_read: (end - start) as u64,
        next_offset,
        eof,
        text,
        encoding: snapshot.encoding.clone(),
        session_modified_at: snapshot.mtime_ms,
    })
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LargeFileInfo {
    pub session_id: String,
    pub size: u64,
    pub mtime: u64,
    pub encoding: String,
    pub is_text: bool,
    pub suggested_chunk_bytes: u64,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadRangeResult {
    /// 本次返回文本对应的源字节数。
    pub bytes_read: u64,
    /// 下一次读取的起始偏移（已对齐到字符边界）。
    pub next_offset: u64,
    pub eof: bool,
    /// 已按会话编码转码为 UTF-8 的文本。
    pub text: String,
    pub encoding: String,
    /// 会话打开时的文件 mtime（毫秒）。
    pub session_modified_at: Option<u64>,
}

// ---------------------------------------------------------------------------
// 分块边界与按编码解码（src-tauri 未依赖 featherview-core 前的内联实现，
// 与 crates/featherview-core 中同名函数保持一致）
// ---------------------------------------------------------------------------

/// 按编码计算分块的有效字节范围（相对 `bytes` 的 [start, end)）。
///
/// `offset` 是请求偏移相对 `bytes` 起点的位置；返回的 start 保证是完整字符
/// 边界（offset 落在字符中间时回退到该字符起点），end 是最后一个完整字符边界。
fn chunk_bounds(bytes: &[u8], offset: usize, encoding: &str) -> (usize, usize) {
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
fn utf8_chunk_bounds(bytes: &[u8], offset: usize) -> (usize, usize) {
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
fn utf16_chunk_bounds(bytes: &[u8], offset: usize, big_endian: bool) -> (usize, usize) {
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
fn gbk_chunk_bounds(bytes: &[u8], offset: usize) -> (usize, usize) {
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

/// 按会话检测到的编码解码为 UTF-8（BOM 手动剥离）。
fn decode_with_encoding(bytes: &[u8], encoding: &str) -> Result<String, AppError> {
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
            encoding_rs::Encoding::for_label(name.as_bytes()).ok_or(AppError::UnknownEncoding)?,
            0,
        ),
    };
    let (text, _, _) = enc.decode(&bytes[bom_len..]);
    Ok(text.into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 会话表是进程级 static：所有会话测试必须串行执行并清理，避免互相干扰。
    static SESSION_TEST_LOCK: Mutex<()> = Mutex::new(());

    fn reset_sessions() {
        *SESSIONS.lock().unwrap_or_else(|e| e.into_inner()) = HashMap::new();
    }

    fn temp_path(name: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("featherview-session-{name}-{}", std::process::id()));
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

    /// 按固定块大小从头读到尾，拼接全部 text。
    fn read_all(session_id: &str, file_len: u64, chunk: u32) -> String {
        let mut offset = 0u64;
        let mut out = String::new();
        loop {
            let r = read_range(session_id.to_string(), offset, chunk).unwrap();
            out.push_str(&r.text);
            if r.eof {
                break;
            }
            offset = r.next_offset;
            assert!(offset <= file_len, "next_offset 越界: {offset} > {file_len}");
        }
        out
    }

    #[test]
    fn session_reads_utf8_chunks_equals_full_file() {
        let _guard = SESSION_TEST_LOCK.lock().unwrap();
        reset_sessions();
        let content = "你好，FeatherView！这是一段用于分块测试的中英文混合文本。Hello world 0123456789 结束。"
            .repeat(50);
        let path = write_temp("utf8-chunk", content.as_bytes());
        let info = open_read_session(path.clone()).unwrap();
        assert!(info.is_text);
        assert_eq!(info.encoding, "UTF-8");
        assert_eq!(info.size, content.len() as u64);
        // 7 字节块必然切开 3 字节中文字符：验证边界回退与拼接一致性
        let joined = read_all(&info.session_id, info.size, 7);
        assert_eq!(joined, content);
        close_read_session(info.session_id.clone()).unwrap();
        assert!(matches!(
            read_range(info.session_id, 0, 7),
            Err(AppError::SessionNotFound)
        ));
        cleanup("utf8-chunk");
    }

    #[test]
    fn session_reads_utf16le_chunks_equals_full_file() {
        let _guard = SESSION_TEST_LOCK.lock().unwrap();
        reset_sessions();
        let text = "A🎉B🚀C🌟D 中文混合 UTF-16 分块测试。".repeat(20);
        let mut bytes = vec![0xFF, 0xFE];
        bytes.extend(text.encode_utf16().flat_map(|u| u.to_le_bytes()));
        let path = write_temp("u16le-chunk", &bytes);
        let info = open_read_session(path.clone()).unwrap();
        assert!(info.is_text);
        assert_eq!(info.encoding, "UTF-16LE");
        // 6 字节 = 3 个 u16 单元：必然切开 emoji 代理对，验证跨块合并
        let joined = read_all(&info.session_id, info.size, 6);
        assert_eq!(joined, text);
        close_read_session(info.session_id).unwrap();
        cleanup("u16le-chunk");
    }

    #[test]
    fn session_reads_gbk_chunks_equals_full_file() {
        let _guard = SESSION_TEST_LOCK.lock().unwrap();
        reset_sessions();
        let text = "这是 GBK 编码的中文分块测试。FeatherView 大文件读取，跨字符边界必须正确。";
        let (encoded, _, _) = encoding_rs::GBK.encode(text);
        let path = write_temp("gbk-chunk", &encoded);
        let info = open_read_session(path.clone()).unwrap();
        assert!(info.is_text);
        assert!(
            info.encoding == "GBK" || info.encoding == "gb18030",
            "unexpected encoding: {}",
            info.encoding
        );
        // 5 字节块必然切开 2 字节 GBK 字符
        let joined = read_all(&info.session_id, info.size, 5);
        assert_eq!(joined, text);
        close_read_session(info.session_id).unwrap();
        cleanup("gbk-chunk");
    }

    #[test]
    fn session_rejects_read_after_close() {
        let _guard = SESSION_TEST_LOCK.lock().unwrap();
        reset_sessions();
        let path = write_temp("close", b"hello session");
        let info = open_read_session(path.clone()).unwrap();
        close_read_session(info.session_id.clone()).unwrap();
        // 幂等：重复关闭也成功
        close_read_session(info.session_id.clone()).unwrap();
        let err = read_range(info.session_id, 0, 10).unwrap_err();
        assert!(matches!(err, AppError::SessionNotFound));
        assert_eq!(err.code(), "SESSION_NOT_FOUND");
        cleanup("close");
    }

    #[test]
    fn session_errors_when_file_changed() {
        let _guard = SESSION_TEST_LOCK.lock().unwrap();
        reset_sessions();
        let path = write_temp("changed", b"original content v1");
        let info = open_read_session(path.clone()).unwrap();
        // 修改文件（内容与长度均变化，mtime / size 双校验均命中）
        std::fs::write(&path, b"modified content version 2, longer!").unwrap();
        let err = read_range(info.session_id.clone(), 0, 16).unwrap_err();
        assert!(matches!(err, AppError::FileChanged(_)));
        assert_eq!(err.code(), "FILE_CHANGED");
        // 会话已被自动关闭
        let err2 = read_range(info.session_id, 0, 16).unwrap_err();
        assert!(matches!(err2, AppError::SessionNotFound));
        cleanup("changed");
    }

    #[test]
    fn session_reads_empty_file() {
        let _guard = SESSION_TEST_LOCK.lock().unwrap();
        reset_sessions();
        let path = write_temp("empty", b"");
        let info = open_read_session(path.clone()).unwrap();
        assert!(info.is_text);
        assert_eq!(info.size, 0);
        let r = read_range(info.session_id.clone(), 0, 1024).unwrap();
        assert!(r.eof);
        assert_eq!(r.text, "");
        assert_eq!(r.bytes_read, 0);
        assert_eq!(r.next_offset, 0);
        close_read_session(info.session_id).unwrap();
        cleanup("empty");
    }

    #[test]
    fn session_reads_file_over_10mb() {
        let _guard = SESSION_TEST_LOCK.lock().unwrap();
        reset_sessions();
        // 11MB 文本：超过 read_file 的 10MB 上限，但会话读取不受限
        let mut content = String::new();
        let mut line_no = 0u32;
        while content.len() < 11 * 1024 * 1024 {
            content.push_str(&format!(
                "第 {line_no:06} 行：FeatherView 大文件分块读取测试内容。\n"
            ));
            line_no += 1;
        }
        let path = write_temp("big11", content.as_bytes());
        let info = open_read_session(path.clone()).unwrap();
        assert!(info.is_text);
        assert!(info.size > 10 * 1024 * 1024);
        let joined = read_all(&info.session_id, info.size, 256 * 1024);
        assert_eq!(joined, content);
        close_read_session(info.session_id).unwrap();
        cleanup("big11");
    }

    #[test]
    fn session_evicts_oldest_at_limit() {
        let _guard = SESSION_TEST_LOCK.lock().unwrap();
        reset_sessions();
        // 先开满 32 个会话
        let mut ids = Vec::new();
        for i in 0..MAX_SESSIONS {
            let path = write_temp(&format!("evict-{i}"), format!("dummy file {i}").as_bytes());
            let info = open_read_session(path.clone()).unwrap();
            ids.push(info.session_id);
        }
        // 打开第 33 个（不同路径）→ 淘汰最早打开的 ids[0]
        let p33 = write_temp("evict-33", b"dummy file 33");
        let info33 = open_read_session(p33.clone()).unwrap();
        let err = read_range(ids[0].clone(), 0, 4).unwrap_err();
        assert!(matches!(err, AppError::SessionNotFound));
        // 其余会话仍可用
        let r = read_range(ids[1].clone(), 0, 4).unwrap();
        assert!(!r.text.is_empty());
        close_read_session(info33.session_id).unwrap();
        for i in 0..MAX_SESSIONS {
            cleanup(&format!("evict-{i}"));
        }
        cleanup("evict-33");
    }

    #[test]
    fn session_rejects_binary_file() {
        let _guard = SESSION_TEST_LOCK.lock().unwrap();
        reset_sessions();
        // PNG 魔数 + NUL 字节（无扩展名 → 内容判定二进制）
        let png: Vec<u8> = vec![
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0, 1, 2, 3, 0, 0, 0, 0,
        ];
        let path = write_temp("binary-session", &png);
        let info = open_read_session(path.clone()).unwrap();
        assert!(!info.is_text);
        assert_eq!(info.encoding, "");
        let err = read_range(info.session_id.clone(), 0, 16).unwrap_err();
        assert!(matches!(err, AppError::BinaryFile));
        assert_eq!(err.code(), "FILE_IS_BINARY");
        close_read_session(info.session_id).unwrap();
        cleanup("binary-session");
    }
}