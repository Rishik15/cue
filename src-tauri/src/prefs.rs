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
    /// Saved by the settings window: the theme's role colours as JSON, and the Appearance mode. The overlay follows both.
    pub theme_colors: String,
    pub theme_mode: String,
    /// One entry per line: `Name` or `from -> to` (see `dictation/clean.rs`).
    pub vocabulary: String,
    pub remove_fillers: bool,
}

impl Prefs {
    pub fn theme(&self) -> crate::platform::Theme {
        crate::platform::Theme { colors: self.theme_colors.clone(), mode: self.theme_mode.clone() }
    }

    pub fn load(app: &AppHandle) -> Self {
        let text = |key: &str, default: &str| get_str(app, key).unwrap_or_else(|| default.into());
        Prefs {
            max_record_secs: get_u64(app, "max_record_secs", 60).clamp(5, 60),
            min_record_ms: get_u64(app, "min_record_ms", 300),
            mic_linger_secs: get_u64(app, "mic_linger_secs", 5),
            mic: Some(text("mic_device", "Default")).filter(|m| m != "Default"),
            show_overlay: get_bool(app, "show_overlay", true),
            overlay_top: text("overlay_position", "Bottom") == "Top",
            always_paste: text("insert_method", "Auto") == "Always Paste",
            restore_clipboard: get_bool(app, "restore_clipboard", true),
            speech_model: text("speech_model", crate::speech::DEFAULT_MODEL),
            theme_colors: text("theme_overlay", ""),
            theme_mode: text("theme_mode", "System"),
            vocabulary: text("vocabulary", ""),
            remove_fillers: get_bool(app, "remove_fillers", true),
        }
    }
}
