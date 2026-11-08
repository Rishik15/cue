//! File: recovery.rs
//! Purpose: The one place a dictated text waits when it could not be inserted and could not be copied either, so a failed copy never drops what the user said. It holds the last such text for this session until a later delivery succeeds or the text is copied; replacing an unrecovered text is reported, never silent.
//! Contents: report_failure — the message for a delivery that did not end well, keeping the text when it went nowhere; hold_note — keep a text and say when it replaced another; hold — keep a text (returns whether it replaced an unrecovered one); clear — a delivery or copy succeeded.
//! shortcut: nothing offers the held text to the user yet (a tray "Copy last dictation" item belongs to the tray layer), add a reader here and call it from there.

use std::sync::Mutex;

use tauri::AppHandle;

use crate::platform::InsertError;
use crate::status;

/// The text waiting to be recovered.
static SLOT: Mutex<Option<String>> = Mutex::new(None);

/// Locks the slot, ignoring poison: a panic elsewhere must not hide a waiting text.
fn slot() -> std::sync::MutexGuard<'static, Option<String>> {
    SLOT.lock().unwrap_or_else(|e| e.into_inner())
}

/// Keeps `text` for the session. Input: the text. Returns true when it replaced a different text that was never recovered, so the caller can say so.
pub fn hold(text: &str) -> bool {
    slot().replace(text.to_string()).is_some_and(|old| old != text)
}

/// Keeps `text` for the session and returns the sentence to add to the user's message when that pushed out an earlier unrecovered text (empty otherwise): a replacement is never silent.
pub fn hold_note(text: &str) -> &'static str {
    if hold(text) {
        " An earlier dictated text that was not copied has been replaced."
    } else {
        ""
    }
}

/// Tells the user how a delivery of a dictated text ended. Text that went in, or sits on the clipboard, needs no keeping and clears the slot; text that went nowhere is kept for the session and the sentence says so (and says when it pushed out an earlier one). Inputs: the app handle, the dictated text and the error.
pub fn report_failure(app: &AppHandle, text: &str, e: &InsertError) {
    if e.text_went_in() || e.copied() {
        clear();
        return status::report_as(app, &e.to_string(), e.pill());
    }
    let note = hold_note(text);
    status::report_as(app, &format!("{e} Cue is keeping the text for this session.{note}"), e.pill());
}

/// The held text is no longer needed: it was copied, or a later delivery succeeded.
pub fn clear() {
    slot().take();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One test walks the slot so parallel tests cannot interleave on the shared static: a text is held, replacing a different one is reported, repeating the same one is not, clearing empties it.
    #[test]
    fn the_slot_holds_one_text_and_reports_a_replacement() {
        clear();
        assert!(!hold("first"));
        assert!(!hold("first"));
        assert!(hold("second"));
        assert_eq!(hold_note("third"), " An earlier dictated text that was not copied has been replaced.");
        assert_eq!(hold_note("third"), "");
        clear();
        assert!(!hold("second"));
        clear();
        assert!(slot().is_none());
    }
}
