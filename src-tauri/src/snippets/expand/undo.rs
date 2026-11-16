//! File: snippets/expand/undo.rs
//! Purpose: Takes an expansion back when the user presses Backspace right after it (a setting, off by default): the text is erased and the trigger they typed is put back.
//! Contents: Undo — what to erase and what to restore; eligible — whether an expansion can be undone safely; run — the undo itself, on a worker.
//! Only short single-line expansions qualify: Backspace counts characters, and a pasted or multi-line text (or a caret moved by `{cursor}`) would not be erased exactly.

use std::time::Duration;

use tauri::AppHandle;

use super::super::model::Insert;
use super::output::put;
use crate::platform::Target;
use crate::status;

const MAX_UNDO_CHARS: usize = 100;
/// The user's own Backspace reaches the app a moment after we see it; ours must come after it.
const AFTER_USERS_KEY: Duration = Duration::from_millis(30);

pub struct Undo {
    /// Characters the expansion put in the app, one of which the user's Backspace already removed.
    pub remove: usize,
    /// What the user had typed: the trigger, and the space or punctuation that ended it.
    pub restore: String,
    pub insert: Insert,
    /// The window and field the expansion went into; the undo goes only there.
    pub token: Target,
}

/// Whether an expansion can be undone exactly. Input: the typed text and how far the caret moved back. Returns true for a short single-line text with the caret at its end.
pub fn eligible(text: &str, back: usize) -> bool {
    back == 0 && text.chars().count() <= MAX_UNDO_CHARS && !text.contains(['\n', '\r'])
}

/// Takes the expansion back, on a worker. Input: the app and what to erase and restore. Returns whether the target's content is known afterwards (false after a failed put, which is reported), so `release` does not replay keys typed meanwhile.
pub fn run(app: &AppHandle, undo: &Undo) -> bool {
    std::thread::sleep(AFTER_USERS_KEY);
    match put(undo.remove.saturating_sub(1), &undo.restore, 0, undo.insert, &undo.token) {
        Ok(()) => true,
        Err(e) => {
            status::report(app, &e);
            false
        }
    }
}
