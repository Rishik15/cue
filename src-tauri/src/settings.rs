//! Purpose: Generic persisted settings (JSON store in the app data dir) and launch-at-login.
//! Contents: STORE — shared store file name; get_setting / set_setting — key-value commands;
//! get_autostart / set_autostart — HKCU Run entry via the autostart plugin.

use serde_json::Value;
use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_store::StoreExt;

pub const STORE: &str = "settings.json";

#[tauri::command]
pub fn get_setting(app: AppHandle, key: String) -> Option<Value> {
    app.store(STORE).ok()?.get(key)
}

#[tauri::command]
pub fn set_setting(app: AppHandle, key: String, value: Value) -> Result<(), String> {
    let store = app.store(STORE).map_err(|e| e.to_string())?;
    store.set(key, value);
    store.save().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_autostart(app: AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

#[tauri::command]
pub fn set_autostart(app: AppHandle, on: bool) -> Result<(), String> {
    let launch = app.autolaunch();
    (if on { launch.enable() } else { launch.disable() }).map_err(|e| e.to_string())
}
