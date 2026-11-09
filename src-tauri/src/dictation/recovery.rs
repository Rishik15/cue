//! File: recovery.rs
//! Purpose: The one place a dictated text waits when it could not be inserted and could not be copied either, so a failed copy never drops what the user said. It holds the last such text for this session until that text is copied; an unrelated later success never removes it, and replacing an unrecovered text is reported, never silent.
//! Contents: report_failure — the message for a delivery that did not end well, keeping the text when it went nowhere; hold_note — keep a text and say when it replaced another; hold — keep a text (returns whether it replaced an unrecovered one); copied — that text was copied, so it is no longer needed.
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

/// Tells the user how a delivery of a dictated text ended. Text that went in or sits on the clipboard needs no keeping (a copy also frees the slot when it holds that same text, and nothing else); text that went nowhere is kept for the session and the sentence says so (and says when it pushed out an earlier one). Inputs: the app handle, the dictated text and the error.
pub fn report_failure(app: &AppHandle, text: &str, e: &InsertError) {
    if e.text_went_in() || e.copied() {
        if e.copied() {
            copied(text);
        }
        return status::report_as(app, &e.to_string(), e.pill());
    }
    let note = hold_note(text);
    status::report_as(app, &format!("{e} Cue is keeping the text for this session.{note}"), e.pill());
}

/// shortcut: equality of text is the identity, so a copy of "x" that lands after another failed delivery of the same "x" empties that one too (the text is on the clipboard then); upgrade to a per-hold generation if the same text can fail twice in one window.
/// `text` was copied to the clipboard: the slot is emptied only if it holds that same text, so another text waiting there, or one that replaced it meanwhile, stays. Returns whether the slot was emptied.
pub fn copied(text: &str) -> bool {
    let mut held = slot();
    let same = held.as_deref() == Some(text);
    if same {
        held.take();
    }
    same
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One test walks the slot so parallel tests cannot interleave on the shared static: a text is held, replacing a different one is reported, repeating the same one is not, and only copying the held text empties it; a copy of any other text (an unrelated success, or a stale copy of a replaced text) leaves it.
    #[test]
    fn the_slot_keeps_a_text_until_that_text_is_copied() {
        copied("first");
        copied("second");
        copied("third");
        assert!(!hold("first"));
        assert!(!hold("first"));
        assert!(!copied("unrelated success"));
        assert_eq!(slot().as_deref(), Some("first"), "an unrelated success leaves the older text");
        assert!(hold("second"));
        assert!(!copied("first"), "a stale copy of the replaced text cannot clear the replacement");
        assert_eq!(hold_note("third"), " An earlier dictated text that was not copied has been replaced.");
        assert_eq!(hold_note("third"), "");
        assert!(copied("third"));
        assert!(slot().is_none());
    }
}
