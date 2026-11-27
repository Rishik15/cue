//! File: settings_door/effects.rs
//! Purpose: What a setting the window saved changes beyond the file, listed key by key in one table, so a new or renamed key never gains or loses an effect by the shape of its name.
//! Contents: Effect — one thing a saved key changes; effects — the effects of one key; apply — carry one out.

use serde_json::Value;
use tauri::AppHandle;

/// One thing a saved setting changes besides the file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    /// The snippet expander re-reads its rules.
    Expansion,
    /// A loaded text model applies the new "unload after" time.
    TextIdle,
    /// An open note window is sent the notes and theme settings.
    NotesWindow,
    /// The shortcut engine re-reads every on/off switch.
    Shortcuts,
}

use Effect::{NotesWindow, Shortcuts, TextIdle};

/// The keys outside the Snippets page that have an effect, and what each changes.
pub(super) const TABLE: &[(&str, &[Effect])] = &[
    ("unload_after_secs", &[TextIdle]),
    ("theme_mode", &[NotesWindow]),
    ("theme_family", &[NotesWindow]),
    ("notes_paper", &[NotesWindow]),
    ("notes_checked", &[NotesWindow]),
    ("notes_text_size", &[NotesWindow]),
    ("notes_digits", &[NotesWindow]),
    ("notes_thousands", &[NotesWindow]),
    ("notes_expire_days", &[NotesWindow]),
    ("notes_countdown_alert", &[NotesWindow]),
    ("notes_pomodoro_alert", &[NotesWindow]),
    ("notes_alert_volume", &[NotesWindow]),
    ("notes_enabled", &[NotesWindow, Shortcuts]),
    ("notes_shortcut_on", &[NotesWindow, Shortcuts]),
    ("dictation_enabled", &[Shortcuts]),
    ("selection_enabled", &[Shortcuts]),
    ("define_enabled", &[Shortcuts]),
    ("dictation_shortcut_on", &[Shortcuts]),
    ("selection_shortcut_on", &[Shortcuts]),
    ("grab_shortcut_on", &[Shortcuts]),
    ("define_shortcut_on", &[Shortcuts]),
];

/// The effects of saving one key: every Snippets page key re-reads the expander, any other key what `TABLE` lists. Input: the key. Returns them, empty for a key with no effect.
pub fn effects(key: &str) -> &'static [Effect] {
    if crate::snippets::check_setting(key, &Value::Null).is_some() {
        return &[Effect::Expansion];
    }
    TABLE.iter().find(|(k, _)| *k == key).map_or(&[], |(_, e)| e)
}

/// Carries out one effect. Inputs: the app and the effect.
pub fn apply(app: &AppHandle, effect: Effect) {
    match effect {
        Effect::Expansion => crate::snippets::sync_expansion(app),
        Effect::TextIdle => crate::text::idle_changed(),
        Effect::NotesWindow => crate::notes::start::push(app),
        Effect::Shortcuts => crate::input::refresh_enabled(app),
    }
}

#[cfg(test)]
mod tests {
    use super::{effects, Effect, TABLE};

    /// Every key in the table is one the window may write, so a renamed or removed key cannot keep an effect that never fires.
    #[test]
    fn every_key_with_an_effect_is_a_window_key() {
        for (key, _) in TABLE {
            assert_ne!(super::super::validate(key, &serde_json::Value::Null), Err(format!("Unknown setting: {key}")));
        }
    }

    /// Effects follow the listed key, never the shape of its name: a new `theme_x` or `foo_enabled` changes nothing, the notes switch reaches both the note window and the shortcuts, and a Snippets page key re-reads the expander.
    #[test]
    fn effects_follow_the_key_not_its_name() {
        assert_eq!(effects("theme_x"), &[] as &[Effect]);
        assert_eq!(effects("foo_enabled"), &[] as &[Effect]);
        assert_eq!(effects("foo_shortcut_on"), &[] as &[Effect]);
        assert_eq!(effects("show_overlay"), &[] as &[Effect]);
        assert_eq!(effects("theme_family"), &[Effect::NotesWindow]);
        assert_eq!(effects("notes_enabled"), &[Effect::NotesWindow, Effect::Shortcuts]);
        assert_eq!(effects("grab_shortcut_on"), &[Effect::Shortcuts]);
        assert_eq!(effects("unload_after_secs"), &[Effect::TextIdle]);
        assert_eq!(effects("snippets_mode"), &[Effect::Expansion]);
    }
}
