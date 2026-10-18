//! Purpose: Generic persisted settings (JSON store in the app data dir) and launch-at-login.
//! Contents: STORE — shared store file name; put — write one value from Rust; get_str / get_u64 / get_i64 / get_bool — typed sync reads for Rust callers (wrong type falls back to the default); get_settings / get_theme / set_setting — whole-store read, theme-only read and key write commands; validate — the allow-list: which keys the UI may write and what each may hold;
//! get_autostart / set_autostart — HKCU Run entry via the autostart plugin; enable_autostart_once — Launch at Login on at first run.

use std::collections::HashMap;

use serde_json::Value;
use tauri::{AppHandle, Manager};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_store::StoreExt;

pub const STORE: &str = "settings.json";

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
    let Ok(store) = app.store(STORE) else { return };
    store.set(key, value);
    if let Err(e) = store.save() {
        crate::status::report(app, &format!("Could not save the setting {key}: {e}"));
    }
}

fn one_of(value: &Value, allowed: &[&str]) -> bool {
    value.as_str().is_some_and(|v| allowed.contains(&v))
}

fn int_in(value: &Value, range: std::ops::RangeInclusive<i64>) -> bool {
    value.as_i64().is_some_and(|v| range.contains(&v))
}

/// The family ids in `src/themes.json`, the same file the UI and the native windows read.
fn is_theme_family(id: &str) -> bool {
    static IDS: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    let ids = IDS.get_or_init(|| {
        let themes: Value = serde_json::from_str(include_str!("../../src/themes.json")).unwrap_or_default();
        themes["families"].as_array().map(|f| f.iter().filter_map(|f| f["id"].as_str().map(String::from)).collect()).unwrap_or_default()
    });
    ids.iter().any(|known| known == id)
}

fn text_up_to(value: &Value, max: usize) -> bool {
    value.as_str().is_some_and(|v| v.chars().count() <= max)
}

/// Only the keys the settings window really writes, each with the type and range it means. The webview is the only caller, but a bug or a hostile page in it
/// must not be able to plant arbitrary keys (the shortcuts and the muted device are not here: they are written by Rust alone) or out-of-range values.
pub fn validate(key: &str, value: &Value) -> Result<(), String> {
    let ok = match key {
        "context_paste"
        | "onboarded"
        | "mute_while_recording"
        | "open_on_launch"
        | "remove_fillers"
        | "restore_clipboard"
        | "show_overlay"
        | "snippets_expand"
        | "snippets_undo"
        | "snippets_match_case"
        | "snippets_follow_case"
        | "snippets_inside_words" => value.is_boolean(),
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
        _ => return Err(format!("Unknown setting: {key}")),
    };
    if ok {
        Ok(())
    } else {
        Err(format!("Invalid value for {key}."))
    }
}

/// A settings file that is not valid JSON is moved to `settings.damaged.json` before anything reads it: the store would otherwise ignore it silently and the next save would overwrite it,
/// destroying the evidence and every setting with it. Returns whether it did, so the caller can say so once the tray exists.
pub fn set_aside_damaged(app: &AppHandle) -> bool {
    let Ok(dir) = app.path().app_data_dir() else { return false };
    let file = dir.join(STORE);
    let Ok(bytes) = std::fs::read(&file) else { return false };
    serde_json::from_slice::<Value>(&bytes).is_err() && std::fs::rename(&file, dir.join("settings.damaged.json")).is_ok()
}

#[tauri::command]
pub async fn set_setting(app: AppHandle, key: String, value: Value) -> Result<(), String> {
    validate(&key, &value)?;
    // The store writes its file on save; keep that off the main thread.
    tauri::async_runtime::spawn_blocking(move || {
        let store = app.store(STORE).map_err(|e| e.to_string())?;
        let changed = key.starts_with("snippets_") && store.get(&key).as_ref() != Some(&value);
        store.set(key, value);
        store.save().map_err(|e| e.to_string())?;
        if changed {
            crate::snippets::sync_expansion(&app); // typed triggers follow the switch and the skip list at once
        }
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
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

/// First run (`onboarded` unset): turn Launch at Login on, once. The marker is a file, not a setting: saving a setting starts the store plugin's async runtime (8 idle threads). It keeps a user who turns Launch at Login off before finishing onboarding from having it switched back on.
pub fn enable_autostart_once(app: &AppHandle) {
    // A dev build must not register its own exe in the Run key.
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
        assert!(validate("output_action", &json!("Insert and Copy")).is_ok());
        assert!(validate("output_action", &json!("Both")).is_err());
        assert!(validate("speech_model", &json!("parakeet-v2")).is_ok());
        assert!(validate("speech_model", &json!("nemotron-en")).is_ok());
        assert!(validate("text_model", &json!("qwen2.5-1.5b")).is_ok());
        assert!(validate("text_model", &json!("../../evil")).is_err());
    }

    #[test]
    fn numbers_stay_in_range() {
        assert!(validate("max_record_secs", &json!(60)).is_ok());
        assert!(validate("max_record_secs", &json!(3600)).is_ok());
        assert!(validate("max_record_secs", &json!(3601)).is_err());
        assert!(validate("max_record_secs", &json!(5)).is_err());
        assert!(validate("unload_after_secs", &json!(-1)).is_ok());
        assert!(validate("unload_after_secs", &json!(-2)).is_err());
        assert!(validate("min_record_ms", &json!("300")).is_err());
    }

    #[test]
    fn text_is_bounded() {
        assert!(validate("vocabulary", &json!("x".repeat(20_001))).is_err());
        assert!(validate("theme_family", &json!("default")).is_ok());
        assert!(validate("theme_family", &json!("gruvbox")).is_err());
        assert!(validate("snippets_ending", &json!("-;")).is_ok());
        assert!(validate("snippets_ending", &json!("a\n")).is_err() && validate("snippets_ending", &json!("\u{7}")).is_err());
    }

    #[test]
    fn unknown_keys_and_keys_only_rust_may_write_are_refused() {
        assert!(validate("selection_hotkey", &json!({})).is_err());
        assert!(validate("muted_endpoint", &json!("device")).is_err());
        assert!(validate("__proto__", &json!(1)).is_err());
    }
}
