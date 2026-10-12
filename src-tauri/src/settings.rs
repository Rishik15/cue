//! Purpose: Generic persisted settings (JSON store in the app data dir) and launch-at-login.
//! Contents: STORE — shared store file name; put — write one value from Rust; get_str / get_u64 / get_i64 / get_bool — typed sync reads for Rust callers (wrong type falls back to the default); get_settings / set_setting — whole-store read and key write commands; validate — the allow-list: which keys the UI may write and what each may hold;
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

/// Writes one value from Rust (the UI uses `set_setting`).
pub fn put(app: &AppHandle, key: &str, value: Value) {
    if let Ok(store) = app.store(STORE) {
        store.set(key, value);
        let _ = store.save();
    }
}

fn one_of(value: &Value, allowed: &[&str]) -> bool {
    value.as_str().is_some_and(|v| allowed.contains(&v))
}

fn int_in(value: &Value, range: std::ops::RangeInclusive<i64>) -> bool {
    value.as_i64().is_some_and(|v| range.contains(&v))
}

fn text_up_to(value: &Value, max: usize) -> bool {
    value.as_str().is_some_and(|v| v.chars().count() <= max)
}

/// Only the keys the settings window really writes, each with the type and range it means. The webview is the only caller, but a bug or a hostile page in it
/// must not be able to plant arbitrary keys (the shortcuts and the muted device are not here: they are written by Rust alone) or out-of-range values.
pub fn validate(key: &str, value: &Value) -> Result<(), String> {
    let ok = match key {
        "context_paste" | "mute_while_recording" | "open_on_launch" | "remove_fillers" | "restore_clipboard" | "show_overlay" => {
            value.is_boolean()
        }
        "auto_submit" => one_of(value, &["Disabled", "Enter", "Ctrl+Enter"]),
        "dictation_mode" => one_of(value, &["Hold", "Toggle", "Hold or Toggle"]),
        "insert_method" => one_of(value, &["Auto", "Always Paste"]),
        "output_action" => one_of(value, &["Insert", "Copy"]),
        "overlay_position" => one_of(value, &["Bottom", "Top"]),
        "theme_mode" => one_of(value, &["System", "Light", "Dark"]),
        "max_record_secs" => int_in(value, 5..=60),
        "min_record_ms" => int_in(value, 0..=5_000),
        "mic_linger_secs" => int_in(value, 0..=3_600),
        "unload_after_secs" => int_in(value, -1..=86_400),
        "mic_device" => text_up_to(value, 200),
        "theme_family" => text_up_to(value, 40),
        "vocabulary" => text_up_to(value, 20_000),
        "theme_overlay" => text_up_to(value, 4_000) && value.as_str().is_some_and(|v| serde_json::from_str::<Value>(v).is_ok()),
        "speech_model" => value.as_str().is_some_and(|id| crate::speech::catalog::find(id).is_some()),
        "text_model" => value.as_str().is_some_and(|id| crate::text::catalog::find(id).is_some()),
        _ => return Err(format!("Unknown setting: {key}")),
    };
    if ok {
        Ok(())
    } else {
        Err(format!("Invalid value for {key}."))
    }
}

#[tauri::command]
pub fn set_setting(app: AppHandle, key: String, value: Value) -> Result<(), String> {
    validate(&key, &value)?;
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::validate;

    #[test]
    fn known_keys_take_their_own_kind_of_value() {
        assert!(validate("show_overlay", &json!(true)).is_ok());
        assert!(validate("show_overlay", &json!("yes")).is_err());
        assert!(validate("dictation_mode", &json!("Hold or Toggle")).is_ok());
        assert!(validate("dictation_mode", &json!("Anything")).is_err());
        assert!(validate("auto_submit", &json!("Ctrl+Enter")).is_ok());
        assert!(validate("speech_model", &json!("parakeet-v2")).is_ok());
        assert!(validate("text_model", &json!("qwen2.5-1.5b")).is_ok());
        assert!(validate("text_model", &json!("../../evil")).is_err());
    }

    #[test]
    fn numbers_stay_in_range() {
        assert!(validate("max_record_secs", &json!(60)).is_ok());
        assert!(validate("max_record_secs", &json!(3600)).is_err());
        assert!(validate("unload_after_secs", &json!(-1)).is_ok());
        assert!(validate("unload_after_secs", &json!(-2)).is_err());
        assert!(validate("min_record_ms", &json!("300")).is_err());
    }

    #[test]
    fn text_is_bounded_and_the_overlay_theme_must_be_json() {
        assert!(validate("vocabulary", &json!("x".repeat(20_001))).is_err());
        assert!(validate("theme_overlay", &json!("{\"dark\":{}}")).is_ok());
        assert!(validate("theme_overlay", &json!("not json")).is_err());
    }

    #[test]
    fn unknown_keys_and_keys_only_rust_may_write_are_refused() {
        assert!(validate("selection_hotkey", &json!({})).is_err());
        assert!(validate("muted_endpoint", &json!("device")).is_err());
        assert!(validate("__proto__", &json!(1)).is_err());
    }
}
