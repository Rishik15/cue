//! File: settings/tests.rs
//! Purpose: Protect the settings door: the window can only write keys it really owns, with the type and range each means, so a bug or hostile page cannot plant keys or out-of-range values.
//! Contents: tests of `validate`.

use serde_json::json;

use super::validate;

/// Whether a write is accepted. Inputs: key and value. Returns the verdict.
fn accepted(key: &str, value: serde_json::Value) -> bool {
    validate(key, &value).is_ok()
}

/// A key the window does not own is refused by name, so nothing can be planted.
#[test]
fn unknown_keys_are_refused_with_their_name() {
    assert_eq!(validate("dictation_shortcut", &json!("Ctrl+A")), Err("Unknown setting: dictation_shortcut".into()));
    assert_eq!(validate("__proto__", &json!(true)), Err("Unknown setting: __proto__".into()));
}

/// On/off keys take only booleans; a string "true" or a number is wrong.
#[test]
fn toggles_take_only_booleans() {
    assert!(accepted("show_overlay", json!(false)));
    assert!(!accepted("show_overlay", json!("true")));
    assert!(!accepted("notes_enabled", json!(1)));
    assert_eq!(validate("check_updates", &json!(null)), Err("Invalid value for check_updates.".into()));
}

/// Choice keys take exactly one of their options.
#[test]
fn choices_take_only_listed_options() {
    assert!(accepted("dictation_mode", json!("Hold or Toggle")));
    assert!(!accepted("dictation_mode", json!("hold")));
    assert!(accepted("snippets_mode", json!("delimiter")));
    assert!(!accepted("snippets_insert", json!("typing")));
    assert!(!accepted("overlay_position", json!(3)));
}

/// Numbers keep to their range at both ends and must be whole.
#[test]
fn numbers_keep_to_their_range() {
    for (key, low, high) in
        [("max_record_secs", 30, 3600), ("notes_text_size", 12, 20), ("unload_after_secs", -1, 86_400), ("notes_alert_volume", 0, 100)]
    {
        assert!(accepted(key, json!(low)) && accepted(key, json!(high)), "{key} edges");
        assert!(!accepted(key, json!(low - 1)) && !accepted(key, json!(high + 1)), "{key} outside");
    }
    assert!(!accepted("notes_digits", json!(2.5)));
    assert!(!accepted("min_record_ms", json!("100")));
}

/// Free text keeps to its length (counted in characters) and the ending of a trigger takes no control character.
#[test]
fn text_keys_keep_to_their_length() {
    assert!(accepted("vocabulary", json!("x".repeat(20_000))));
    assert!(!accepted("vocabulary", json!("x".repeat(20_001))));
    assert!(accepted("mic_device", json!("é".repeat(200))));
    assert!(!accepted("mic_device", json!("é".repeat(201))));
    assert!(accepted("snippets_ending", json!(" .,")));
    assert!(!accepted("snippets_ending", json!("a\nb")));
}

/// Models and theme families must exist in their catalogs.
#[test]
fn models_and_themes_must_exist() {
    assert!(!accepted("speech_model", json!("no-such-model")));
    assert!(!accepted("text_model", json!("no-such-model")));
    assert!(!accepted("theme_family", json!("no-such-theme")));
    assert!(!accepted("speech_model", json!(5)));
}
