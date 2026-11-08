//! File: settings/validate.rs
//! Purpose: The settings door: which keys the settings window may write and what each may hold, so a bug or a hostile page in the webview cannot plant keys or out-of-range values.
//! Contents: validate — the allow-list check for one write; the small checks it is built from (choices, integer ranges, text length, theme families, the note window's own keys, on/off keys).

use serde_json::Value;

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
        let themes: Value = serde_json::from_str(include_str!("../../../src/themes.json")).unwrap_or_default();
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
