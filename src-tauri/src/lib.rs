mod commands;
mod document;
mod error;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::file::read_file,
            commands::file::file_exists,
            commands::file::file_metadata,
            commands::system::app_version,
            commands::system::startup_args,
            commands::system::platform_info,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
