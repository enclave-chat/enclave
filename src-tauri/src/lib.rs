pub mod commands;
pub mod types;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::server_list::save_server_list,
            commands::server_list::get_server_list,
            commands::accounts::save_accounts,
            commands::accounts::get_accounts,
            commands::audio::list_input_devices,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
