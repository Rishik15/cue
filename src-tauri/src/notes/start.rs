//! Purpose: What a note window's page needs before it paints: the first note, how it looks (the notes settings) and the theme.
//! Contents: Start — the data written into the page; options — the notes settings with their defaults; push — send the current settings to an open note window.

use serde::Serialize;
use tauri::AppHandle;

use super::Open;
use crate::settings;

/// What the page needs before it paints: the note, how it looks and the theme.
#[derive(Serialize)]
pub struct Start {
    pub open: Open,
    pub options: Options,
    /// The first note's math results, so its first paint shows them.
    pub answers: super::eval::Evaluated,
    pub theme: std::collections::HashMap<String, serde_json::Value>,
    /// The guide page to open with: "first", the new version, or nothing.
    pub welcome: Option<String>,
}

#[derive(Serialize)]
pub struct Options {
    text_size: u64,
    paper: String,
    checked: String,
    pub(super) digits: u64,
    pub(super) thousands: bool,
}

pub fn options(app: &AppHandle) -> Options {
    Options {
        text_size: settings::get_u64(app, "notes_text_size", 14),
        paper: settings::get_str(app, "notes_paper").unwrap_or_else(|| "Plain".into()),
        checked: settings::get_str(app, "notes_checked").unwrap_or_else(|| "Stay".into()),
        digits: settings::get_u64(app, "notes_digits", 2),
        thousands: settings::get_bool(app, "notes_thousands", true),
    }
}

/// Tells an open note window the settings as they are now (called when a `notes_` or `theme_` setting is saved), so a change shows at once; nothing happens when no note is open.
pub fn push(app: &AppHandle) {
    if !super::window::exists(app) {
        return;
    }
    let payload = serde_json::json!({ "options": options(app), "theme": settings::get_theme(app.clone()) });
    let _ = tauri::Emitter::emit_to(app, super::window::WINDOW, "notes-settings", payload);
}
