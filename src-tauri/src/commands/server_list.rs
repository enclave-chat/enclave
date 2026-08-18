use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs};
use tauri::{AppHandle, Manager};

use crate::protocol::ServerMeta;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnownServer {
    pub meta: ServerMeta,
    pub public_key: String,
    pub is_secure: bool,
}

type ServerList = HashMap<String, KnownServer>;

fn servers_file_path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to resolve app data dir: {e}"))?;

    // ensure the directory exists before we ever try to read/write into it
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create app data dir: {e}"))?;

    Ok(dir.join("servers.json"))
}

#[tauri::command]
pub fn save_server_list(app: AppHandle, servers: ServerList) -> Result<(), String> {
    let path = servers_file_path(&app)?;

    let json = serde_json::to_string_pretty(&servers)
        .map_err(|e| format!("Failed to serialize server list: {e}"))?;

    fs::write(&path, json).map_err(|e| format!("Failed to write server list: {e}"))?;

    Ok(())
}

#[tauri::command]
pub fn get_server_list(app: AppHandle) -> Result<ServerList, String> {
    let path = servers_file_path(&app)?;

    if !path.exists() {
        fs::write(&path, "[]").map_err(|e| format!("Failed to write first server list: {e}"))?;

        return Ok(ServerList::new());
    }

    let json = fs::read_to_string(&path).map_err(|e| format!("Failed to read server list: {e}"))?;

    serde_json::from_str(&json).map_err(|e| format!("Failed to parse server list: {e}"))
}
