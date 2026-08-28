use cpal::traits::{DeviceTrait, HostTrait};
use serde::{Deserialize, Serialize};
use std::fs;
use std::sync::Mutex;
use tauri::{AppHandle, Manager, State};

use crate::commands::audio::VoiceState;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub input_device_name: Option<String>,
    pub output_device_name: Option<String>,
    pub input_volume: u8,
    pub output_volume: u8,
}

impl Default for Config {
    fn default() -> Self {
        let host = cpal::default_host();

        Self {
            input_device_name: host
                .default_input_device()
                .and_then(|d| Some(d.description().ok()?.name().to_string())),
            output_device_name: host
                .default_output_device()
                .and_then(|d| Some(d.description().ok()?.name().to_string())),
            input_volume: 0,
            output_volume: 0,
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
pub fn update_config(
    state: State<ConfigState>,
    voice_state: State<'_, VoiceState>,
    mut config: Config,
) -> Result<(), String> {
    let mut state_lock = state.0.lock().unwrap();

    std::mem::swap(&mut *state_lock, &mut config);

    let Some(sesh) = &mut *voice_state.inner.session.lock().unwrap() else {
        return Ok(());
    };

    if state_lock.input_device_name != config.input_device_name {
        sesh.setup_input(
            state_lock.input_device_name.clone(),
            voice_state.inner.clone(),
        )?;
    }

    if state_lock.output_device_name != config.output_device_name {
        sesh.setup_output(
            state_lock.output_device_name.clone(),
            voice_state.inner.clone(),
        )?;
    }

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

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendConfig {
    pub is_muted: bool,
    pub is_deaf: bool,
}

#[tauri::command]
pub fn update_backend_config(
    voice_state: State<'_, VoiceState>,
    config: BackendConfig,
) -> Result<(), String> {
    if let Some(v) = &mut *voice_state.inner.session.lock().unwrap() {
        *v.backend_config.lock().unwrap() = config
    }

    Ok(())
}
