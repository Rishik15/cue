//! File: busy.rs
//! Purpose: One answer to "is something already using the screen or the keyboard?" for every shortcut press, so two features never read or type at once (a second Ctrl+C from the wrong window, a card over the palette).
//! Contents: busy — dictating, the palette, the strip or the question window open, a lookup or card up, a selection or grab in progress; busy_with — which of those it is, for a log line.

use tauri::AppHandle;

use crate::{dictation, dictionary, grab, preview, selection, snippets, strip};

/// Whether a feature already uses the screen or keyboard. Input: app handle. Returns true when a shortcut press must be ignored.
pub fn busy(app: &AppHandle) -> bool {
    busy_with(app).is_some()
}

/// The first feature that already uses the screen or keyboard. Input: app handle. Returns its name, or None when nothing does.
pub fn busy_with(app: &AppHandle) -> Option<&'static str> {
    [
        ("dictation", dictation::is_recording()),
        ("preview", preview::is_open(app)),
        ("strip", strip::is_open(app)),
        ("lookup", dictionary::is_looking()),
        ("selection", selection::is_busy()),
        ("grab", grab::is_busy()),
        ("snippet question", snippets::ask::is_open()),
    ]
    .into_iter()
    .find_map(|(name, busy)| busy.then_some(name))
}
