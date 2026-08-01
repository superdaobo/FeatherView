mod commands;
mod document;
mod error;

use tauri::{Emitter, Manager};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(any(debug_assertions, feature = "agent-mode"))]
    if let Err(e) = commands::agent_mode::start_if_requested() {
        eprintln!("[agent-mode] failed to start: {e}");
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            // 恢复并聚焦主窗口（最小化时解除）
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
            // 将第二实例参数转发给前端统一处理
            let _ = app.emit("external-file-open", args);
        }))
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
