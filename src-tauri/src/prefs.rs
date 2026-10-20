//! Purpose: The user's dictation preferences as one typed snapshot, so features read settings in one place.
//! Contents: Prefs — recording, overlay, insertion and speech options; load — read them from the store with defaults.
//! Keys and defaults must match the Voice and General pages in `src/pages`.

use tauri::AppHandle;

use crate::settings::{get_bool, get_str, get_u64};

pub struct Prefs {
    pub max_record_secs: u64,
    pub min_record_ms: u64,
    pub mic_linger_secs: u64,
    pub mic: Option<String>,
    pub show_overlay: bool,
    pub overlay_top: bool,
    pub always_paste: bool,
    pub restore_clipboard: bool,
    pub speech_model: String,
    /// The theme family id and the Appearance mode, both saved by the settings window. Native windows follow them.
    pub theme_family: String,
    pub theme_mode: String,
    /// One entry per line: `Name` or `from -> to` (see `dictation/clean.rs`).
    pub vocabulary: String,
    pub remove_fillers: bool,
    /// Output Action "Copy": the text goes to the clipboard instead of the focused app.
    pub output_copy: bool,
    /// Output Action "Insert and Copy": the text goes into the focused app and is then left on the clipboard too.
    pub keep_copy: bool,
    pub context_paste: bool,
    /// "Disabled", "Enter" or "Ctrl+Enter": a key pressed after the text is inserted.
    pub auto_submit: String,
}

impl Prefs {
    pub fn theme(&self) -> crate::platform::Theme {
        crate::platform::Theme { family: self.theme_family.clone(), mode: self.theme_mode.clone() }
    }

    pub fn load(app: &AppHandle) -> Self {
        let text = |key: &str, default: &str| get_str(app, key).unwrap_or_else(|| default.into());
        Prefs {
            max_record_secs: get_u64(app, "max_record_secs", 1200).clamp(30, 3600),
            min_record_ms: get_u64(app, "min_record_ms", 300).min(5_000),
            mic_linger_secs: get_u64(app, "mic_linger_secs", 5).min(3_600),
            mic: Some(text("mic_device", "Default")).filter(|m| m != "Default"),
            show_overlay: get_bool(app, "show_overlay", true),
            overlay_top: text("overlay_position", "Bottom") == "Top",
            always_paste: text("insert_method", "Auto") == "Always Paste",
            restore_clipboard: get_bool(app, "restore_clipboard", true),
            speech_model: text("speech_model", crate::speech::default_model()),
            theme_family: text("theme_family", "default"),
            theme_mode: text("theme_mode", "System"),
            vocabulary: text("vocabulary", ""),
            remove_fillers: get_bool(app, "remove_fillers", true),
            output_copy: text("output_action", "Insert") == "Copy",
            keep_copy: text("output_action", "Insert") == "Insert and Copy",
            context_paste: get_bool(app, "context_paste", false),
            auto_submit: text("auto_submit", "Disabled"),
        }
    }
}
