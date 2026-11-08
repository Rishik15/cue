//! File: settings.rs
//! Purpose: Generic persisted settings (JSON store in the app data dir), their version guard, and launch-at-login.
//! shortcut: the store holds a flat key map stamped with `VERSION`; there are no migrations yet, add a step in `commit` before the stamp when a key changes meaning. Typed descriptors per key (one table for default, range and effect) are not built: `validate` and the callers' defaults stay separate.
//! Contents: STORE — shared store file name; VERSION — the layout version this build writes; is_newer — the store was written by a newer Cue; NEWER — the refusal text; commit — save with the version stamp; get_str / get_u64 / get_i64 / get_bool — typed sync reads for Rust callers (wrong type falls back to the default); get_settings / get_theme / set_setting — whole-store read, theme-only read and key write commands; validate — the allow-list: which keys the UI may write and what each may hold;
//! get_autostart / set_autostart — HKCU Run entry via the autostart plugin; enable_autostart_once — Launch at Login on at first run.

use std::collections::HashMap;

use serde_json::Value;
use tauri::{AppHandle, Manager};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_store::StoreExt;

pub const STORE: &str = "settings.json";
/// The layout version this build writes into the store under `settings_version`. A store stamped higher belongs to a newer Cue and is read but never written.
pub const VERSION: u64 = 1;
/// The largest settings file Cue reads; a bigger one is treated as damaged, so a hostile or runaway file cannot multiply startup allocations.
const MAX_FILE_BYTES: u64 = 1 << 20;
const VERSION_KEY: &str = "settings_version";
/// Held for a whole write (read the old value, set, save, roll back), so two overlapping writes to one key cannot restore each other's stale value.
static WRITING: std::sync::Mutex<()> = std::sync::Mutex::new(());
/// What the settings window is told when a write is refused because the store is from a newer Cue.
pub const NEWER: &str = "These settings were saved by a newer version of Cue. Update Cue to change them.";

/// Whether the store was written by a newer Cue. Input: the app handle. Returns true when its version stamp is above `VERSION`.
pub fn is_newer(app: &AppHandle) -> bool {
    get_u64(app, VERSION_KEY, 0) > VERSION
}

/// Saves the store with the version stamp; every writer of the store ends with it. Input: the open store. Returns the save error as text.
pub fn commit(store: &tauri_plugin_store::Store<tauri::Wry>) -> Result<(), String> {
    store.set(VERSION_KEY, VERSION);
    store.save().map_err(|e| e.to_string())
}

/// Whole store in one call so the UI can seed every signal before first paint.
#[tauri::command]
pub fn get_settings(app: AppHandle) -> HashMap<String, Value> {
    app.store(STORE).map(|s| s.entries().into_iter().collect()).unwrap_or_default()
}

/// Only the `theme_*` keys, for the preview and question windows: they paint the theme and must not see vocabulary, the microphone or other settings.
#[tauri::command]
pub fn get_theme(app: AppHandle) -> HashMap<String, Value> {
    get_settings(app).into_iter().filter(|(k, _)| k.starts_with("theme_")).collect()
}

/// One stored value. Input: the app handle and key. Returns it, or None when absent or the store cannot be opened.
pub fn get(app: &AppHandle, key: &str) -> Option<Value> {
    app.store(STORE).ok()?.get(key)
}

/// A stored string. Input: the app handle and key. Returns None when absent or not a string.
pub fn get_str(app: &AppHandle, key: &str) -> Option<String> {
    get(app, key)?.as_str().map(String::from)
}

/// Stores a value Cue itself decided on (not one the UI wrote, so it skips `validate`). Inputs: the app handle, key and value. A failed save only means it is decided again next launch; a store from a newer Cue is left alone.
pub fn remember(app: &AppHandle, key: &str, value: Value) {
    if is_newer(app) {
        return;
    }
    if let Ok(store) = app.store(STORE) {
        let _writing = WRITING.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        store.set(key, value);
        let _ = commit(&store);
    }
}

/// A stored unsigned integer. Input: the app handle, key and default. Wrong-typed or missing values give the default, so a hand-edited store cannot break a feature.
pub fn get_u64(app: &AppHandle, key: &str, default: u64) -> u64 {
    get(app, key).and_then(|v| v.as_u64()).unwrap_or(default)
}

/// A stored integer. Input: the app handle, key and default. Returns the default when absent or not an integer.
pub fn get_i64(app: &AppHandle, key: &str, default: i64) -> i64 {
    get(app, key).and_then(|v| v.as_i64()).unwrap_or(default)
}

/// A stored boolean. Input: the app handle, key and default. Returns the default when absent or not a boolean.
pub fn get_bool(app: &AppHandle, key: &str, default: bool) -> bool {
    get(app, key).and_then(|v| v.as_bool()).unwrap_or(default)
}

/// Whether the value is a string in the allowed list. Input: value and the list. Returns the verdict.
fn one_of(value: &Value, allowed: &[&str]) -> bool {
    value.as_str().is_some_and(|v| allowed.contains(&v))
}

/// Whether the value is an integer in the range. Input: value and range. Returns the verdict.
fn int_in(value: &Value, range: std::ops::RangeInclusive<i64>) -> bool {
    value.as_i64().is_some_and(|v| range.contains(&v))
}

/// Whether an id is a theme family in `src/themes.json`, the file the UI and the native windows read. Input: the id. Returns the verdict.
fn is_theme_family(id: &str) -> bool {
    static IDS: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    let ids = IDS.get_or_init(|| {
        let themes: Value = serde_json::from_str(include_str!("../../src/themes.json")).unwrap_or_default();
        themes["families"].as_array().map(|f| f.iter().filter_map(|f| f["id"].as_str().map(String::from)).collect()).unwrap_or_default()
    });
    ids.iter().any(|known| known == id)
}

/// Whether the value is a string of at most `max` characters. Input: value and the limit. Returns the verdict.
fn text_up_to(value: &Value, max: usize) -> bool {
    value.as_str().is_some_and(|v| v.chars().count() <= max)
}

/// Checks one of the note window's own keys. Inputs: key and value. Returns None when the key is not one of them, else whether the value is allowed.
fn notes_value(key: &str, value: &Value) -> Option<bool> {
    Some(match key {
        "notes_paper" => one_of(value, &["Plain", "Dots", "Small Grid", "Medium Grid"]),
        "notes_checked" => one_of(value, &["Stay", "Move", "Delete"]),
        "notes_text_size" => int_in(value, 12..=20),
        "notes_digits" => int_in(value, 0..=7),
        "notes_expire_days" => int_in(value, 0..=365),
        "notes_countdown_alert" | "notes_pomodoro_alert" => one_of(value, &["Notification", "Full Screen", "Sound"]),
        "notes_alert_volume" => int_in(value, 0..=100),
        _ => return None,
    })
}

/// Whether a key is an on/off key the settings window may write (values are checked as booleans). Input: the key. Returns the verdict. Feature toggles (`<name>_enabled`) and per-shortcut toggles (`<name>_shortcut_on`) also re-sync the shortcut engine in `set_setting`.
fn boolean_key(key: &str) -> bool {
    matches!(
        key,
        "context_paste"
            | "onboarded"
            | "mute_while_recording"
            | "open_on_launch"
            | "check_updates"
            | "remove_fillers"
            | "restore_clipboard"
            | "show_overlay"
            | "snippets_expand"
            | "snippets_undo"
            | "snippets_match_case"
            | "snippets_follow_case"
            | "snippets_inside_words"
            | "dictation_enabled"
            | "selection_enabled"
            | "define_enabled"
            | "notes_enabled"
            | "dictation_shortcut_on"
            | "selection_shortcut_on"
            | "grab_shortcut_on"
            | "define_shortcut_on"
            | "notes_shortcut_on"
            | "notes_thousands"
    )
}

/// Checks a write from the settings window. Inputs: key and value. Returns Err with a message for an unknown key or a value of the wrong type or range. Only the keys the window really writes pass, each with the type and range it means. The webview is the only caller, but a bug or a hostile page in it
/// must not be able to plant arbitrary keys (the shortcuts are not here: they are written by Rust alone) or out-of-range values.
pub fn validate(key: &str, value: &Value) -> Result<(), String> {
    let ok = match key {
        k if boolean_key(k) => value.is_boolean(),
        "auto_submit" => one_of(value, &["Disabled", "Enter", "Ctrl+Enter"]),
        "dictation_mode" => one_of(value, &["Hold", "Toggle", "Hold or Toggle"]),
        "insert_method" => one_of(value, &["Auto", "Always Paste"]),
        "output_action" => one_of(value, &["Insert", "Copy", "Insert and Copy"]),
        "overlay_position" => one_of(value, &["Bottom", "Top"]),
        "theme_mode" => one_of(value, &["System", "Light", "Dark"]),
        "max_record_secs" => int_in(value, 30..=3600),
        "onboarding_step" => int_in(value, 0..=20),
        "min_record_ms" => int_in(value, 0..=5_000),
        "mic_linger_secs" => int_in(value, 0..=3_600),
        "unload_after_secs" => int_in(value, -1..=86_400),
        "mic_device" => text_up_to(value, 200),
        "theme_family" => value.as_str().is_some_and(is_theme_family),
        "vocabulary" => text_up_to(value, 20_000),
        "snippets_skip_apps" => text_up_to(value, 4_000),
        "snippets_ending" => text_up_to(value, 20) && value.as_str().is_some_and(|v| !v.chars().any(char::is_control)),
        "snippets_mode" => one_of(value, &["auto", "immediate", "delimiter"]),
        "snippets_insert" => one_of(value, &["auto", "paste", "slow"]),
        "speech_model" => value.as_str().is_some_and(|id| crate::speech::catalog::find(id).is_some()),
        "text_model" => value.as_str().is_some_and(|id| crate::text::catalog::find(id).is_some()),
        _ => match notes_value(key, value) {
            Some(ok) => ok,
            None => return Err(format!("Unknown setting: {key}")),
        },
    };
    if ok {
        Ok(())
    } else {
        Err(format!("Invalid value for {key}."))
    }
}

/// Moves a damaged settings file aside. Input: the app handle. A settings file that is not valid JSON, or is larger than `MAX_FILE_BYTES` (checked before it is read), is moved to `settings.damaged.json` before anything reads it: the store would otherwise ignore it silently and the next save would overwrite it,
/// destroying the evidence and every setting with it. Returns whether it did, so the caller can say so once the tray exists.
pub fn set_aside_damaged(app: &AppHandle) -> bool {
    let Ok(dir) = app.path().app_data_dir() else { return false };
    let file = dir.join(STORE);
    if std::fs::metadata(&file).is_ok_and(|m| m.len() > MAX_FILE_BYTES) {
        return std::fs::rename(&file, dir.join("settings.damaged.json")).is_ok();
    }
    let Ok(bytes) = std::fs::read(&file) else { return false };
    serde_json::from_slice::<Value>(&bytes).is_err() && std::fs::rename(&file, dir.join("settings.damaged.json")).is_ok()
}

/// Writes one validated setting from the UI and applies its effect. Inputs: key and value. Returns Err with a message when validation fails, the store is from a newer Cue, or the save fails; the UI restores its previous value then.
#[tauri::command]
pub async fn set_setting(app: AppHandle, key: String, value: Value) -> Result<(), String> {
    validate(&key, &value)?;
    if is_newer(&app) {
        return Err(NEWER.into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let store = app.store(STORE).map_err(|e| e.to_string())?;
        let pushes_to_notes = key.starts_with("notes_") || key.starts_with("theme_");
        let toggles_feature = key.ends_with("_enabled") || key.ends_with("_shortcut_on");
        let changed = {
            let _writing = WRITING.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            let previous = store.get(&key);
            let changed = key.starts_with("snippets_") && previous.as_ref() != Some(&value);
            store.set(&key, value);
            if let Err(e) = commit(&store) {
                match previous {
                    Some(old) => store.set(&key, old),
                    None => {
                        store.delete(&key);
                    }
                }
                return Err(e);
            }
            changed
        };
        if changed {
            crate::snippets::sync_expansion(&app);
        }
        if pushes_to_notes {
            crate::notes::start::push(&app);
        }
        if toggles_feature {
            crate::shortcut::refresh_enabled(&app);
        }
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Whether Launch at Login is on. Input: the app handle. Returns false when it cannot be read.
#[tauri::command]
pub fn get_autostart(app: AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

/// Turns Launch at Login on or off. Inputs: the app handle and the wanted state. Returns Err with the system's message on failure.
#[tauri::command]
pub fn set_autostart(app: AppHandle, on: bool) -> Result<(), String> {
    let launch = app.autolaunch();
    (if on { launch.enable() } else { launch.disable() }).map_err(|e| e.to_string())
}

/// First run (`onboarded` unset): in a release build only (a dev build must not register its own exe in the Run key), turn Launch at Login on, once. The marker is a file, not a setting: saving a setting starts the store plugin's async runtime (8 idle threads). It keeps a user who turns Launch at Login off before finishing onboarding from having it switched back on.
pub fn enable_autostart_once(app: &AppHandle) {
    if cfg!(debug_assertions) || get(app, "onboarded").is_some() {
        return;
    }
    let Ok(dir) = app.path().app_data_dir() else { return };
    let marker = dir.join("autostart.seeded");
    if marker.exists() || std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(&marker, b"")).is_err() {
        return;
    }
    if let Err(e) = app.autolaunch().enable() {
        crate::status::report(app, &format!("Could not turn on Launch at Login: {e}. Turn it on in Settings."));
    }
}

#[cfg(test)]
mod tests;
