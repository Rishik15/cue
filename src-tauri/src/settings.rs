//! Purpose: Generic persisted settings (JSON store in the app data dir) and launch-at-login.
//! Contents: STORE — shared store file name; get_str / get_u64 / get_i64 / get_bool — typed sync reads for Rust callers (wrong type falls back to the default); get_settings / set_setting — whole-store read and key write commands;
//! get_autostart / set_autostart — HKCU Run entry via the autostart plugin.

use std::collections::HashMap;

use serde_json::Value;
use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_store::StoreExt;

pub const STORE: &str = "settings.json";

/// Whole store in one call so the UI can seed every signal before first paint.
#[tauri::command]
pub fn get_settings(app: AppHandle) -> HashMap<String, Value> {
    app.store(STORE).map(|s| s.entries().into_iter().collect()).unwrap_or_default()
}

fn get(app: &AppHandle, key: &str) -> Option<Value> {
    app.store(STORE).ok()?.get(key)
}

pub fn get_str(app: &AppHandle, key: &str) -> Option<String> {
    get(app, key)?.as_str().map(String::from)
}

/// Wrong-typed or missing values fall back to the default, so a hand-edited store cannot break a feature.
pub fn get_u64(app: &AppHandle, key: &str, default: u64) -> u64 {
    get(app, key).and_then(|v| v.as_u64()).unwrap_or(default)
}

pub fn get_i64(app: &AppHandle, key: &str, default: i64) -> i64 {
    get(app, key).and_then(|v| v.as_i64()).unwrap_or(default)
}

pub fn get_bool(app: &AppHandle, key: &str, default: bool) -> bool {
    get(app, key).and_then(|v| v.as_bool()).unwrap_or(default)
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
