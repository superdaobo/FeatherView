mod commands;
mod document;
mod error;

use tauri::{Emitter, Manager};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init());

    // 单实例插件仅桌面平台可用（Android/iOS 不支持）
    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            // 恢复并聚焦主窗口（最小化时解除）
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
            // 将第二实例参数转发给前端统一处理
            let _ = app.emit("external-file-open", args);
        }));
    }

    #[cfg(any(debug_assertions, feature = "agent-mode"))]
    if let Err(e) = commands::agent_mode::start_if_requested() {
        eprintln!("[agent-mode] failed to start: {e}");
    }

    builder
        .invoke_handler(tauri::generate_handler![
            commands::file::read_file,
            commands::file::file_exists,
            commands::file::file_metadata,
            commands::file::open_read_session,
            commands::file::read_range,
            commands::file::close_read_session,
            commands::system::app_version,
            commands::system::startup_args,
            commands::system::platform_info,
            commands::uri::read_uri,
            commands::uri::copy_uri_to_temp,
            commands::uri::uri_temp_info,
            commands::preview::preview_handler_status,
            commands::preview::install_preview_handler,
            commands::preview::uninstall_preview_handler,
            commands::preview::file_association_status,
            commands::preview::install_file_association,
            commands::preview::remove_file_association,
            commands::preview::resolve_preview_dll,
            commands::preview::app_exe_path,
            commands::preview::preview_ping,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
