//! File: recovery.rs
//! Purpose: The one place a dictated text waits when it could not be inserted and could not be copied either, so a failed copy never drops what the user said. It holds the last such text for this session until a later delivery succeeds or the text is copied; replacing an unrecovered text is reported, never silent.
//! Contents: hold — keep a text (returns whether it replaced an unrecovered one); clear — a delivery or copy succeeded.
//! shortcut: nothing offers the held text to the user yet (a tray "Copy last dictation" item belongs to the tray layer), add a reader here and call it from there.

use std::sync::Mutex;

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
        clear();
        assert!(!hold("second"));
        clear();
        assert!(slot().is_none());
    }
}
