//! File: selection.rs
//! Purpose: Read the text selected in the focused app by copying it, without disturbing what the user had on the clipboard, and without waiting for the shortcut's keys to come up.
//! Contents: copy_selection — borrow the clipboard (refused when it cannot be put back), lift the held modifiers, Ctrl+C, wait for the clipboard to change, read the text and any HTML (bounded), put the old clipboard back unless the user copied something newer; how long the wait is depends on whether the field already says nothing is selected.
//! press_copy / read_text / uia_fallback — the steps of that flow.

use std::time::{Duration, Instant};

use windows::Win32::UI::Input::KeyboardAndMouse::{VK_C, VK_CONTROL};

use super::borrow::{Borrow, Ours};
use super::clipboard::{look_within, sequence, Looked, TooLarge, HTML_BYTES, TEXT_BYTES};
use super::context::{field_state, selection_in, FieldState, Session};
use super::elevation;
use super::input::Lift;
use super::insert::{key_chord, send};
use crate::platform::Copied;

/// Apps answer Ctrl+C within a few milliseconds; a few (spreadsheets, remote desktops) render lazily and a busy machine is slower still, so allow longer. Only the "nothing selected" case waits the full time.
const COPY_TIMEOUT: Duration = Duration::from_millis(2000);
/// The wait when the focused field itself says nothing is selected: a fast app has answered by then, and "nothing selected" shows in a fraction of the time.
const EMPTY_TIMEOUT: Duration = Duration::from_millis(150);
/// The same wait on a small PC, which answers more slowly.
const EMPTY_TIMEOUT_WEAK: Duration = Duration::from_millis(400);
/// The longest selection read through UI Automation when the clipboard cannot be borrowed.
const MAX_UIA_CHARS: usize = 200_000;

/// Copies the selection and returns it. `Ok(None)` when nothing is selected (the copy changed nothing). Err with a line for the user when the field is a password, the app is elevated, the selection is too long, or the clipboard cannot be borrowed and UI Automation does not know the text.
pub fn copy_selection() -> Result<Option<Copied>, String> {
    if elevation::target_blocks_input() {
        return Err("This app runs as administrator, so Cue cannot read its selection.".into());
    }
    let begun = Instant::now();
    let session = Session::open();
    let timeout = match field_state(session.as_ref()) {
        FieldState::Password => return Err("Cue does not read from password fields.".into()),
        FieldState::Empty if super::hardware::weak_machine() => EMPTY_TIMEOUT_WEAK,
        FieldState::Empty => EMPTY_TIMEOUT,
        FieldState::Other => COPY_TIMEOUT,
    };
    let borrow = match Borrow::begin() {
        Ok(borrow) => borrow,
        Err(why) => return uia_fallback(session.as_ref(), why),
    };
    let changed = press_copy(timeout)?;
    crate::log::note(&format!("selection: {} in {} ms", if changed { "copied" } else { "none" }, begun.elapsed().as_millis()));
    if !changed {
        return Ok(None);
    }
    let looked = read_copy();
    let restored = borrow
        .finish(Ours::Sequence(looked.as_ref().map_or_else(|_| sequence(), |l| l.sequence)))
        .inspect_err(|why| crate::log::note(&format!("selection: could not put the clipboard back: {why}")))
        .is_ok();
    let Looked { text, html, .. } = looked?;
    Ok(text.filter(|t| !t.is_empty()).map(|text| Copied { text, html, restored }))
}

/// The selection as UI Automation knows it (without its formatting) when the clipboard cannot be borrowed; otherwise `why` it cannot be read.
fn uia_fallback(session: Option<&Session>, why: String) -> Result<Option<Copied>, String> {
    session
        .and_then(|s| selection_in(s, MAX_UIA_CHARS))
        .map(|found| Some(Copied { text: found.text, html: None, restored: true }))
        .ok_or(why)
}

/// Sends Ctrl+C with the shortcut's held keys lifted and waits up to `timeout` for the clipboard to change. True when it did.
fn press_copy(timeout: Duration) -> Result<bool, String> {
    let before = sequence();
    {
        let _lift = Lift::new();
        send(&key_chord(VK_CONTROL, VK_C))?;
    }
    let sent = Instant::now();
    while sequence() == before && sent.elapsed() < timeout {
        std::thread::sleep(Duration::from_millis(5));
    }
    Ok(sequence() != before)
}

/// Looks at the copy the app made: its text, its HTML and the sequence number that proves nothing was copied since, all taken in one open of the clipboard. The app may still hold the clipboard open (or not have rendered the text) for a moment after the sequence number moved, so a busy clipboard or a missing text is retried for about 200 ms.
/// That one open is also why no settling wait is needed: the app's writes are finished once the clipboard can be opened, and no other program can write while it is open. Err when the text is larger than `TEXT_BYTES`; the last look (possibly without text) otherwise.
fn read_copy() -> Result<Looked, String> {
    let mut last = None;
    for _ in 0..=20 {
        match look_within(TEXT_BYTES, HTML_BYTES) {
            Ok(Some(looked)) if looked.text.is_some() => return Ok(looked),
            Ok(Some(looked)) => last = Some(looked),
            Ok(None) => {}
            Err(TooLarge) => return Err("The selection is too long.".into()),
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    Ok(last.unwrap_or(Looked { sequence: sequence(), text: None, html: None }))
}
