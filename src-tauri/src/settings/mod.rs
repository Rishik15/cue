//! File: settings/mod.rs
//! Purpose: Cue's persisted settings: one JSON file in the app data folder, read once into a shared snapshot, changed only through one serialized, atomic update, with the window's write door (`validate`) and Launch at Login.
//! shortcut: the file is a flat key map stamped with `VERSION`; `file::MIGRATIONS` is empty until a key changes meaning (add a step there, the version follows). Typed descriptors per key (one table for default, range and effect) are not built: `validate` and the callers' defaults stay separate.
//! shortcut: the rename that replaces the file is not followed by a directory flush; NTFS journals it, upgrade to `MOVEFILE_WRITE_THROUGH` in `platform/` if a power-loss report appears.
//! Contents: init — read the file at startup; get / get_str / get_u64 / get_i64 / get_bool — typed reads of the snapshot (wrong type falls back to the default); update / remember — the only writers; is_newer / NEWER — the read-only state; get_settings / get_theme / set_setting — the window's commands; validate — the allow-list; autostart — Launch at Login.
//! Nothing here polls or keeps a thread: the file is read once and written only when a setting changes, and no change is announced to windows that did not ask.

pub mod autostart;
mod file;
mod store;
mod validate;

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use serde::{Serialize, Serializer};
use serde_json::Value;
use tauri::{AppHandle, Manager};

pub use autostart::enable_autostart_once;
pub use file::Map;
pub use validate::validate;

use store::Store;

pub const STORE: &str = "settings.json";

static STORE_CELL: OnceLock<Store> = OnceLock::new();
static EMPTY: OnceLock<Arc<Map>> = OnceLock::new();

/// Reads the settings file once, at startup, before anything reads a setting. Input: the app handle. Returns what to tell the user once the tray exists (the file was damaged and set aside, or cannot be used), or None.
pub fn init(app: &AppHandle) -> Option<String> {
    let dir: PathBuf = app.path().app_data_dir().ok()?;
    let (store, notice) = Store::open(dir.join(STORE));
    let _ = STORE_CELL.set(store);
    notice
}

/// The current settings. Returns a shared snapshot, empty before `init`.
fn snapshot() -> Arc<Map> {
    STORE_CELL.get().map_or_else(|| Arc::clone(EMPTY.get_or_init(|| Arc::new(Map::new()))), Store::snapshot)
}

/// Whether the settings were written by a newer Cue and so are read-only. Returns the verdict.
pub fn is_newer() -> bool {
    STORE_CELL.get().is_some_and(Store::is_newer)
}

/// One stored value. Input: the key. Returns a copy, or None when absent.
pub fn get(key: &str) -> Option<Value> {
    snapshot().get(key).cloned()
}

/// A stored string. Input: the key. Returns None when absent or not a string.
pub fn get_str(key: &str) -> Option<String> {
    snapshot().get(key)?.as_str().map(String::from)
}

/// A stored unsigned integer. Inputs: the key and a default. Wrong-typed or missing values give the default, so a hand-edited file cannot break a feature.
pub fn get_u64(key: &str, default: u64) -> u64 {
    snapshot().get(key).and_then(Value::as_u64).unwrap_or(default)
}

/// A stored integer. Inputs: the key and a default. Returns the default when absent or not an integer.
pub fn get_i64(key: &str, default: i64) -> i64 {
    snapshot().get(key).and_then(Value::as_i64).unwrap_or(default)
}

/// A stored boolean. Inputs: the key and a default. Returns the default when absent or not a boolean.
pub fn get_bool(key: &str, default: bool) -> bool {
    snapshot().get(key).and_then(Value::as_bool).unwrap_or(default)
}

/// Changes several keys in one atomic save; the only way any code writes a setting. Input: the changes (Some sets a key, None removes it). Returns Err with a user-readable text when the settings are read-only or cannot be saved; nothing changes then.
pub fn update(changes: &[(&str, Option<Value>)]) -> Result<(), String> {
    STORE_CELL.get().ok_or_else(|| "The settings are not ready yet.".to_string())?.update(changes)
}

/// Stores a value Cue itself decided on (not one the UI wrote, so it skips `validate`). Inputs: key and value. A failed save only means it is decided again next launch.
pub fn remember(key: &str, value: Value) {
    let _ = update(&[(key, Some(value))]);
}

/// The settings snapshot as JSON, without copying the map.
pub struct Snapshot(Arc<Map>);

impl Serialize for Snapshot {
    /// Serializes the map. Input: the serializer. Returns its result.
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

/// Whole settings in one call so the UI can seed every signal before first paint.
#[tauri::command]
pub fn get_settings() -> Snapshot {
    Snapshot(snapshot())
}

/// Only the `theme_*` keys, for the preview and question windows: they paint the theme and must not see vocabulary, the microphone or other settings.
#[tauri::command]
pub fn get_theme() -> Map {
    snapshot().iter().filter(|(k, _)| k.starts_with("theme_")).map(|(k, v)| (k.clone(), v.clone())).collect()
}

/// Writes one validated setting from the UI and applies its effect. Inputs: key and value. Returns Err with a message when validation fails, the settings are read-only or the save fails; the UI restores its previous value then.
#[tauri::command]
pub async fn set_setting(app: AppHandle, key: String, value: Value) -> Result<(), String> {
    validate(&key, &value)?;
    tauri::async_runtime::spawn_blocking(move || {
        update(&[(key.as_str(), Some(value))])?;
        if key.starts_with("snippets_") {
            crate::snippets::sync_expansion(&app);
        }
        if key.starts_with("notes_") || key.starts_with("theme_") {
            crate::notes::start::push(&app);
        }
        if key.ends_with("_enabled") || key.ends_with("_shortcut_on") {
            crate::input::refresh_enabled(&app);
        }
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests;
