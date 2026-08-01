/// 系统级命令（预留扩展点）。
use crate::error::AppError;

/// 返回应用版本号（用于关于信息展示）。
#[tauri::command]
pub fn app_version() -> Result<String, AppError> {
    Ok(env!("CARGO_PKG_VERSION").to_string())
}

/// 返回进程启动参数（用于启动参数/文件关联打开入口）。
#[tauri::command]
pub fn startup_args() -> Vec<String> {
    std::env::args().collect()
}

/// 预留：平台信息。移动端可在此扩展 content:// 与 file:// 的处理。
#[tauri::command]
pub fn platform_info() -> Result<PlatformInfo, AppError> {
    #[cfg(target_os = "windows")]
    let os = "windows";
    #[cfg(target_os = "android")]
    let os = "android";
    #[cfg(target_os = "ios")]
    let os = "ios";
    #[cfg(target_os = "macos")]
    let os = "macos";
    #[cfg(target_os = "linux")]
    let os = "linux";

    Ok(PlatformInfo {
        os: os.to_string(),
        tauri_version: tauri::VERSION.to_string(),
    })
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformInfo {
    pub os: String,
    pub tauri_version: String,
}
