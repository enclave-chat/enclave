use serde::{Deserialize, Serialize};
use std::fs;
use std::sync::Mutex;
use tauri::{AppHandle, Manager, State};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub audio_device_name: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            audio_device_name: None,
        }
    }
}

pub struct ConfigState(pub Mutex<Config>);

fn config_file_path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to resolve app data dir: {e}"))?;
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create app data dir: {e}"))?;
    Ok(dir.join("config.json"))
}

#[tauri::command]
pub fn update_config(state: State<ConfigState>, config: Config) -> Result<(), String> {
    *state.0.lock().unwrap() = config;
    Ok(())
}

#[tauri::command]
pub fn save_config(app: AppHandle, state: State<ConfigState>) -> Result<(), String> {
    let path = config_file_path(&app)?;
    let config = state.0.lock().unwrap();

    let json = serde_json::to_string_pretty(&*config)
        .map_err(|e| format!("Failed to serialize config: {e}"))?;

    fs::write(&path, json).map_err(|e| format!("Failed to write config: {e}"))?;

    Ok(())
}

#[tauri::command]
pub fn get_config(app: AppHandle, state: State<ConfigState>) -> Result<Config, String> {
    let path = config_file_path(&app)?;

    let config = if path.exists() {
        let json = fs::read_to_string(&path).map_err(|e| format!("Failed to read config: {e}"))?;
        serde_json::from_str(&json).map_err(|e| format!("Failed to parse config: {e}"))?
    } else {
        Config::default()
    };

    *state.0.lock().unwrap() = config.clone();

    Ok(config)
}
