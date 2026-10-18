//! Purpose: Read the text selected in the focused app by copying it, without disturbing what the user had on the clipboard, and without waiting for the shortcut's keys to come up.
//! Contents: copy_selection — borrow the clipboard (refused when it cannot be put back), lift the held modifiers, Ctrl+C, wait for the clipboard to change, read the text and any HTML, put the old clipboard back; how long the wait is depends on whether the field already says nothing is selected.

use std::time::{Duration, Instant};

use windows::Win32::UI::Input::KeyboardAndMouse::{VK_C, VK_CONTROL};

use super::clipboard::{html_fragment, restore, sequence, snapshot, unicode_text, BORROW_BYTES};
use super::context::{field_state, selection_in, FieldState, Session};
use super::elevation;
use super::input::Lift;
use super::insert::{key_chord, send};
use crate::platform::Copied;

/// Apps answer Ctrl+C within a few milliseconds; a few (spreadsheets, remote desktops) render lazily and a busy machine is slower still, so allow longer. Only the "nothing selected" case waits the full time.
const COPY_TIMEOUT: Duration = Duration::from_millis(1000);
/// The wait when the focused field itself says nothing is selected: a fast app has answered by then, and "nothing selected" shows in a fraction of the time.
const EMPTY_TIMEOUT: Duration = Duration::from_millis(150);

/// The longest selection read through UI Automation when the clipboard cannot be borrowed.
const MAX_UIA_CHARS: usize = 200_000;

/// `Ok(None)` when nothing is selected (the copy changed nothing).
pub fn copy_selection() -> Result<Option<Copied>, String> {
    if elevation::target_blocks_input() {
        return Err("This app runs as administrator, so Cue cannot read its selection.".into());
    }
    let begun = Instant::now();
    let session = Session::open(); // one session for the whole flow, also for the fallback below
    let timeout = match field_state(session.as_ref()) {
        FieldState::Password => return Err("Cue does not read from password fields.".into()),
        FieldState::Empty => EMPTY_TIMEOUT,
        FieldState::Other => COPY_TIMEOUT,
    };
    // A clipboard that cannot be put back is never borrowed: UI Automation may still know the text (without its formatting), else the user is told.
    let saved = match snapshot(BORROW_BYTES) {
        Ok(saved) => saved,
        Err(why) => {
            return session
                .as_ref()
                .and_then(|s| selection_in(s, MAX_UIA_CHARS))
                .map(|found| Copied { text: found.text, html: None })
                .map(Some)
                .ok_or(why)
        }
    };
    let before = sequence();
    {
        let _lift = Lift::new(); // the shortcut's keys may still be down: Ctrl+C must arrive as Ctrl+C
        send(&key_chord(VK_CONTROL, VK_C))?;
    }
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
    if let Err(why) = restore(saved) {
        crate::log::note(&format!("selection: could not put the clipboard back: {why}"));
    }
    Ok(text.filter(|t| !t.is_empty()).map(|text| Copied { text, html }))
}
