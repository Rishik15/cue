//! File: settings/mod.rs
//! Purpose: Cue's persisted settings: one JSON file in the app data folder, read once into a shared snapshot, changed only through one serialized, atomic update, and Launch at Login. It knows no feature: the window's write door (`settings_door`) checks writes and carries out their effects.
//! shortcut: the file is a flat key map stamped with `VERSION`; a newer file is read-only. Typed descriptors per key (one table for default, range and effect) are not built: `settings_door::validate`, its effect table and the callers' defaults stay separate.
//! shortcut: the rename that replaces the file is not followed by a directory flush; NTFS journals it, upgrade to `MOVEFILE_WRITE_THROUGH` in `platform/` if a power-loss report appears.
//! Contents: init — read the file at startup; is_ready — whether init ran; get / get_str / get_u64 / get_i64 / get_bool — typed reads of the snapshot (wrong type falls back to the default); update / remember — the only writers; is_newer / NEWER — the read-only state; get_settings / get_theme — the window's read commands (get_theme sends only `THEME_KEYS`); UNLOAD_DEFAULT_SECS — the idle-unload default; autostart — Launch at Login.
//! Nothing here polls or keeps a thread: the file is read once and written only when a setting changes, and no change is announced to windows that did not ask.

pub mod autostart;
mod file;
mod store;

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use serde::{Serialize, Serializer};
use serde_json::Value;
use tauri::{AppHandle, Manager};

pub use autostart::enable_autostart_once;
pub use file::Map;

use store::Store;

pub const STORE: &str = "settings.json";
/// Seconds an idle model stays loaded when `unload_after_secs` is not set; the speech and text engines both read it.
pub const UNLOAD_DEFAULT_SECS: i64 = 120;
/// The keys the preview and question windows may read (`get_theme`): a new theme key is added here by name.
const THEME_KEYS: [&str; 2] = ["theme_mode", "theme_family"];

static STORE_CELL: OnceLock<Store> = OnceLock::new();
static EMPTY: OnceLock<Arc<Map>> = OnceLock::new();

/// Reads the settings file once, at startup, before anything reads a setting. Input: the app handle. Returns what to tell the user once the tray exists (the file was damaged and set aside, or cannot be used), or None.
pub fn init(app: &AppHandle) -> Option<String> {
    let dir: PathBuf = app.path().app_data_dir().ok()?;
    let (store, notice) = Store::open(dir.join(STORE));
    let _ = STORE_CELL.set(store);
    notice
}

/// Whether `init` has read the settings; false before it and in a start refused before it. Returns the answer.
pub fn is_ready() -> bool {
    STORE_CELL.get().is_some()
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

/// Only the `THEME_KEYS`, for the preview and question windows: they paint the theme and must not see vocabulary, the microphone or other settings.
#[tauri::command]
pub fn get_theme() -> Map {
    theme_of(&snapshot())
}

/// The `THEME_KEYS` entries of a settings map. Input: the map. Returns a map holding only those keys.
fn theme_of(map: &Map) -> Map {
    map.iter().filter(|(k, _)| THEME_KEYS.contains(&k.as_str())).map(|(k, v)| (k.clone(), v.clone())).collect()
}

#[cfg(test)]
mod tests {
    use super::{theme_of, Map};
    use serde_json::json;

    /// Only the named theme keys reach the preview and question windows; a key that merely looks like one does not.
    #[test]
    fn theme_keeps_only_named_keys() {
        let mut map = Map::new();
        for key in ["theme_mode", "theme_family", "theme_x", "vocabulary"] {
            map.insert(key.into(), json!("v"));
        }
        let mut keys: Vec<String> = theme_of(&map).keys().cloned().collect();
        keys.sort();
        assert_eq!(keys, ["theme_family", "theme_mode"]);
    }
}
