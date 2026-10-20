//! Purpose: What a note window's page needs before it paints: the first note, how it looks (the notes settings) and the theme.
//! Contents: Start — the data written into the page; options — the notes settings with their defaults.

use serde::Serialize;
use tauri::AppHandle;

use super::Open;
use crate::settings;

/// What the page needs before it paints: the note, how it looks and the theme.
#[derive(Serialize)]
pub struct Start {
    pub open: Open,
    pub options: Options,
    pub theme: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Serialize)]
pub struct Options {
    text_size: u64,
    paper: String,
    checked: String,
    digits: u64,
    thousands: bool,
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
