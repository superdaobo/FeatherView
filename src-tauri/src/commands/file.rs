use crate::document::{self, ReadFileResult};
use crate::error::AppError;

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
