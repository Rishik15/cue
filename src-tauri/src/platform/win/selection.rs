//! Purpose: Read the text selected in the focused app by copying it, without disturbing what the user had on the clipboard.
//! Contents: copy_selection — Ctrl+C, wait for the clipboard to change, read the text and any HTML, put the old clipboard back; how long the wait is depends on whether the field already says nothing is selected.

use std::time::{Duration, Instant};

use windows::Win32::UI::Input::KeyboardAndMouse::{VK_C, VK_CONTROL};

use super::clipboard::{html_fragment, restore, sequence, snapshot, unicode_text};
use super::context::selection_is_empty;
use super::elevation;
use super::insert::{key_chord, send, wait_modifiers_released};
use crate::platform::Copied;

/// Apps answer Ctrl+C within a few milliseconds; a few (spreadsheets, remote desktops) render lazily and a busy machine is slower still, so allow longer. Only the "nothing selected" case waits the full time.
const COPY_TIMEOUT: Duration = Duration::from_millis(1000);
/// The wait when the focused field itself says nothing is selected: a fast app has answered by then, and "nothing selected" shows in a fraction of the time.
const EMPTY_TIMEOUT: Duration = Duration::from_millis(150);

/// `Ok(None)` when nothing is selected (the copy changed nothing).
pub fn copy_selection() -> Result<Option<Copied>, String> {
    if elevation::target_blocks_input() {
        return Err("This app runs as administrator, so Cue cannot read its selection.".into());
    }
    if !wait_modifiers_released(Duration::from_millis(600), false) {
        // Ctrl+C would arrive as Ctrl+Alt+C or similar.
        return Err("Release the shortcut keys, then press the shortcut again.".into());
    }
    let begun = Instant::now();
    let timeout = if selection_is_empty() { EMPTY_TIMEOUT } else { COPY_TIMEOUT };
    let saved = snapshot();
    let before = sequence();
    send(&key_chord(VK_CONTROL, VK_C))?;
    let sent = Instant::now();
    while sequence() == before && sent.elapsed() < timeout {
        std::thread::sleep(Duration::from_millis(5));
    }
    let changed = sequence() != before;
    crate::log::note(&format!("selection: {} in {} ms", if changed { "copied" } else { "none" }, begun.elapsed().as_millis()));
    if !changed {
        return Ok(None);
    }
    let (text, html) = (unicode_text(), html_fragment());
    restore(saved);
    Ok(text.filter(|t| !t.is_empty()).map(|text| Copied { text, html }))
}
