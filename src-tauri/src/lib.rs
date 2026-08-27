use std::sync::Mutex;

use crate::commands::config::ConfigState;

pub mod commands;
pub mod types;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(ConfigState(Mutex::new(commands::config::Config::default())))
        .manage(commands::audio::VoiceState::default())
        .invoke_handler(tauri::generate_handler![
            commands::server_list::save_server_list,
            commands::server_list::get_server_list,
            commands::accounts::save_accounts,
            commands::accounts::get_accounts,
            commands::audio::list_input_devices,
            commands::audio::list_output_devices,
            commands::audio::connect_to_vc,
            commands::audio::disconnect_from_vc,
            commands::config::update_config,
            commands::config::save_config,
            commands::config::get_config,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
