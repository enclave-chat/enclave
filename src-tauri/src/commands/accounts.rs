use serde::{Deserialize, Serialize};
use std::fs;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub display_name: String,
    pub private_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountsFile {
    pub active_account: Option<usize>,
    pub accounts: Vec<Account>,
}

fn accounts_file_path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to resolve app data dir: {e}"))?;
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create app data dir: {e}"))?;
    Ok(dir.join("accounts.json"))
}

#[tauri::command]
pub fn save_accounts(app: AppHandle, data: AccountsFile) -> Result<(), String> {
    let path = accounts_file_path(&app)?;
    let json = serde_json::to_string_pretty(&data)
        .map_err(|e| format!("Failed to serialize accounts: {e}"))?;
    fs::write(&path, json).map_err(|e| format!("Failed to write accounts: {e}"))?;
    Ok(())
}

#[tauri::command]
pub fn get_accounts(app: AppHandle) -> Result<AccountsFile, String> {
    let path = accounts_file_path(&app)?;
    if !path.exists() {
        return Ok(AccountsFile {
            active_account: None,
            accounts: Vec::new(),
        });
    }
    let json = fs::read_to_string(&path).map_err(|e| format!("Failed to read accounts: {e}"))?;
    serde_json::from_str(&json).map_err(|e| format!("Failed to parse accounts: {e}"))
}
