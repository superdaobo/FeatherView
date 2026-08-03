//! 预览处理器与文件关联管理命令。
//!
//! 通过 winreg 操作 HKCU 注册表（免管理员），
//! 让用户在主应用设置界面中自行管理：
//! - Windows 资源管理器预览处理器（右侧预览窗格）
//! - 文件关联（双击/打开方式）

use std::process::Command;
use winreg::enums::{HKEY_CURRENT_USER, KEY_READ};
use winreg::RegKey;

/// 预览处理器 CLSID（与 crates/windows-preview-handler 一致）
const CLSID: &str = "{8895b1c6-b41f-4c1c-a562-0d564250836f}";
/// PreviewHandler 注册 GUID（shellex 下的固定键名）
const PREVIEW_HANDLER_GUID: &str = "{8895b1c6-b41f-4c1c-a562-0d564250836f}";
/// 支持预览的扩展名
const PREVIEW_EXTENSIONS: &[&str] = &[
    "md", "markdown", "mdown", "txt", "log", "json", "yaml", "yml", "toml", "csv",
];
/// 支持文件关联的扩展名
const ASSOCIATION_EXTENSIONS: &[&str] = &[
    "md", "markdown", "mdown", "txt", "log", "json", "yaml", "yml", "toml",
];

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewHandlerStatus {
    pub installed: bool,
    pub dll_path: Option<String>,
    pub extensions: Vec<String>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssociationStatus {
    pub extension: String,
    pub associated: bool,
}

/// HKCU\Software\Classes 前缀
const CLASSES_PREFIX: &str = "Software\\Classes\\";

fn reg_query(path: &str, name: &str) -> Option<String> {
    let full = format!("{CLASSES_PREFIX}{path}");
    let key = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(&full, KEY_READ)
        .ok()?;
    let value_name = if name == "(default)" { "" } else { name };
    key.get_value::<String, _>(value_name).ok()
}

fn reg_add(path: &str, name: &str, value: &str) -> Result<(), String> {
    let full = format!("{CLASSES_PREFIX}{path}");
    let key = RegKey::predef(HKEY_CURRENT_USER)
        .create_subkey(&full)
        .map_err(|e| format!("创建注册表键失败: {e}"))?
        .0;
    let value_name = if name == "(default)" { "" } else { name };
    key.set_value(value_name, &value)
        .map_err(|e| format!("写入注册表失败: {e}"))
}

fn reg_delete_key(path: &str) -> Result<(), String> {
    let full = format!("{CLASSES_PREFIX}{path}");
    match RegKey::predef(HKEY_CURRENT_USER).delete_subkey_all(&full) {
        Ok(_) => Ok(()),
        Err(_) => Ok(()), // 键不存在也视为成功（幂等）
    }
}

/// 解析预览处理器 DLL 路径（资源目录 / exe 同目录 / 开发目录）。
#[tauri::command]
pub fn resolve_preview_dll(app: tauri::AppHandle) -> Option<String> {
    use tauri::Manager;
    // 1. tauri 资源目录（打包后 DLL 随应用分发）
    if let Ok(dir) = app.path().resource_dir() {
        let p = dir.join("lanxia_preview_handler.dll");
        if p.is_file() {
            return p.to_str().map(|s| s.to_string());
        }
    }
    // 2. exe 同目录
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let p = dir.join("lanxia_preview_handler.dll");
            if p.is_file() {
                return p.to_str().map(|s| s.to_string());
            }
        }
    }
    // 3. 开发模式：仓库 src-tauri/resources（相对进程工作目录）
    for candidate in [
        "src-tauri/resources/lanxia_preview_handler.dll",
        "resources/lanxia_preview_handler.dll",
    ] {
        if std::path::Path::new(candidate).is_file() {
            if let Ok(abs) = std::path::absolute(candidate) {
                return abs.to_str().map(|s| s.to_string());
            }
        }
    }
    None
}

/// 返回主程序 exe 路径（用于文件关联命令）。
#[tauri::command]
pub fn app_exe_path() -> Result<String, String> {
    std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .map_err(|e| format!("无法获取主程序路径: {e}"))
}

/// 调试：最简单命令，验证 Tauri 命令通道。
#[tauri::command]
pub fn preview_ping() -> String {
    "pong".to_string()
}

/// 查询预览处理器安装状态。
#[tauri::command]
pub fn preview_handler_status() -> PreviewHandlerStatus {
    let clsid_key = format!(r"CLSID\{CLSID}\InprocServer32");
    let dll_path = reg_query(&clsid_key, "(default)");
    let installed = dll_path.is_some();
    let mut extensions = Vec::new();
    if installed {
        for ext in PREVIEW_EXTENSIONS {
            let key = format!(r".{ext}\shellex\{PREVIEW_HANDLER_GUID}");
            if reg_query(&key, "(default)").is_some() {
                extensions.push((*ext).to_string());
            }
        }
    }
    PreviewHandlerStatus {
        installed,
        dll_path,
        extensions,
    }
}

/// 安装预览处理器：注册 CLSID 并为支持扩展名注册 PreviewHandler（HKCU）。
#[tauri::command]
pub async fn install_preview_handler(dll_path: String) -> Result<(), String> {
    if !std::path::Path::new(&dll_path).is_file() {
        return Err(format!("找不到预览处理器 DLL：{dll_path}"));
    }
    let clsid_key = format!(r"CLSID\{CLSID}\InprocServer32");
    reg_add(&clsid_key, "(default)", &dll_path)?;
    reg_add(&clsid_key, "ThreadingModel", "Apartment")?;
    for ext in PREVIEW_EXTENSIONS {
        let key = format!(r".{ext}\shellex\{PREVIEW_HANDLER_GUID}");
        reg_add(&key, "(default)", CLSID)?;
    }
    // 提示 shell 刷新
    let _ = Command::new("ie4uinit.exe").arg("-show").output();
    Ok(())
}

/// 卸载预览处理器。
#[tauri::command]
pub async fn uninstall_preview_handler() -> Result<(), String> {
    for ext in PREVIEW_EXTENSIONS {
        let key = format!(r".{ext}\shellex\{PREVIEW_HANDLER_GUID}");
        let _ = reg_delete_key(&key);
    }
    let clsid_key = format!(r"CLSID\{CLSID}");
    let _ = reg_delete_key(&clsid_key);
    let _ = Command::new("ie4uinit.exe").arg("-show").output();
    Ok(())
}

/// 查询文件关联状态。
#[tauri::command]
pub async fn file_association_status() -> Vec<AssociationStatus> {
    ASSOCIATION_EXTENSIONS
        .iter()
        .map(|ext| {
            let prog_id_key = format!(r".{ext}\OpenWithProgids\FeatherView.md");
            // 仅按扩展名自己的 OpenWithProgids 判定（ProgID command 是全局的）
            let associated = reg_query(&prog_id_key, "(default)").is_some();
            AssociationStatus {
                extension: (*ext).to_string(),
                associated,
            }
        })
        .collect()
}

/// 安装文件关联（"打开方式"加入览匣），exe_path 为主程序路径。
#[tauri::command]
pub async fn install_file_association(
    exe_path: String,
    extensions: Vec<String>,
) -> Result<(), String> {
    if !std::path::Path::new(&exe_path).is_file() {
        return Err(format!("找不到主程序：{exe_path}"));
    }
    let quoted = format!("\"{}\" \"%1\"", exe_path);
    // ProgID 打开命令
    reg_add(r"FeatherView.md\shell\open\command", "(default)", &quoted)?;
    for ext in &extensions {
        let prog_id_key = format!(r".{ext}\OpenWithProgids\FeatherView.md");
        reg_add(&prog_id_key, "(default)", "")?;
    }
    let _ = Command::new("ie4uinit.exe").arg("-show").output();
    Ok(())
}

/// 移除文件关联。
#[tauri::command]
pub async fn remove_file_association(extensions: Vec<String>) -> Result<(), String> {
    for ext in &extensions {
        let prog_id_key = format!(r".{ext}\OpenWithProgids\FeatherView.md");
        let _ = reg_delete_key(&prog_id_key);
    }
    // 若所有扩展名都移除，清理 ProgID（保留命令键无害，此处保留以支持再次添加）
    let _ = Command::new("ie4uinit.exe").arg("-show").output();
    Ok(())
}

#[cfg(test)]
mod preview_debug_tests {
    #[test]
    fn test_reg_query_direct() {
        let out = std::process::Command::new("reg")
            .args(["query", r"HKCU\Software\Classes\CLSID\{8895b1c6-b41f-4c1c-a562-0d564250836f}\InprocServer32", "/ve"])
            .output()
            .expect("reg should run");
        println!(
            "status={} stdout={} stderr={}",
            out.status,
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(out.status.success());
    }
}
#[cfg(test)]
mod winreg_debug_tests {
    use winreg::enums::*;
    use winreg::RegKey;
    #[test]
    fn test_winreg_read() {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let classes = hkcu
            .open_subkey_with_flags("Software\\Classes", KEY_READ)
            .expect("open Classes");
        let clsid = classes
            .open_subkey_with_flags(
                r"CLSID\{8895b1c6-b41f-4c1c-a562-0d564250836f}\InprocServer32",
                KEY_READ,
            )
            .expect("open CLSID");
        let v: String = clsid.get_value("").expect("get default");
        println!("VALUE=[{v}]");
        assert!(!v.is_empty());
    }
}
