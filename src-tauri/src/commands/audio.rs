use cpal::traits::{DeviceTrait, HostTrait};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioDevice {
    pub name: String,
}

#[tauri::command]
pub fn list_input_devices() -> Result<Vec<AudioDevice>, String> {
    let host = cpal::default_host();

    let devices = host
        .input_devices()
        .map_err(|e| format!("Failed to enumerate input devices: {e}"))?;

    Ok(devices
        .filter_map(|d| {
            Some(AudioDevice {
                name: d.description().ok()?.name().to_string(),
            })
        })
        .collect())
}
