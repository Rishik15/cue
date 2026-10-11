//! Purpose: Read the text selected in the focused app by copying it, without disturbing what the user had on the clipboard.
//! Contents: copy_selection — Ctrl+C, wait for the clipboard to change, read the text, put the old clipboard back.

use std::time::{Duration, Instant};

use windows::Win32::UI::Input::KeyboardAndMouse::{VK_C, VK_CONTROL};

use super::clipboard::{restore, sequence, snapshot, unicode_text};
use super::elevation;
use super::insert::{key_chord, send, wait_modifiers_released};

/// Apps answer Ctrl+C within a few milliseconds; a few (spreadsheets, remote desktops) render lazily and a busy machine is slower still, so allow longer. Only the "nothing selected" case waits the full time.
const COPY_TIMEOUT: Duration = Duration::from_millis(1000);

/// `Ok(None)` when nothing is selected (the copy changed nothing).
pub fn copy_selection() -> Result<Option<String>, String> {
    if elevation::target_blocks_input() {
        return Err("This app runs as administrator, so Cue cannot read its selection.".into());
    }
    if !wait_modifiers_released(Duration::from_millis(600)) {
        // Ctrl+C would arrive as Ctrl+Alt+C or similar.
        return Err("Release the shortcut keys, then press the shortcut again.".into());
    }
    let saved = snapshot();
    let before = sequence();
    send(&key_chord(VK_CONTROL, VK_C))?;
    let start = Instant::now();
    while sequence() == before && start.elapsed() < COPY_TIMEOUT {
        std::thread::sleep(Duration::from_millis(10));
    }
    if sequence() == before {
        return Ok(None);
    }
    let text = unicode_text();
    restore(saved);
    Ok(text.filter(|t| !t.is_empty()))
}
