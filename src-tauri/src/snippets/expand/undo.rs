//! File: snippets/expand/undo.rs
//! Purpose: Takes an expansion back when the user presses Backspace right after it (a setting, off by default): the text is erased and the trigger they typed is put back.
//! Contents: Undo — what to erase and what to restore; eligible — whether an expansion can be undone safely; run — the undo itself, on a worker.
//! Only short single-line expansions qualify: Backspace counts characters, and a pasted or multi-line text (or a caret moved by `{cursor}`) would not be erased exactly.

use crate::lock::lock;
use std::sync::atomic::Ordering;
use std::time::Duration;

use tauri::AppHandle;

use super::super::model::Insert;
use super::flow::Outcome;
use super::output::{put, Hooks};
use super::{still_current, Shared};
use crate::platform::Target;
use crate::status;

const MAX_UNDO_CHARS: usize = 100;
/// The user's own Backspace reaches the app a moment after we see it; ours must come after it.
const AFTER_USERS_KEY: Duration = Duration::from_millis(30);

/// How to take back the last expansion when the user presses Backspace right after it.
pub struct Undo {
    /// Characters the expansion put in the app, one of which the user's Backspace already removed.
    pub remove: usize,
    /// What the user had typed: the trigger, and the space or punctuation that ended it.
    pub restore: String,
    pub insert: Insert,
    /// The Response Time wait of a paste.
    pub settle: Duration,
    /// The window and field the expansion went into; the undo goes only there.
    pub token: Target,
}

/// Whether an expansion can be undone exactly. Input: the typed text and how far the caret moved back. Returns true for a short single-line text with the caret at its end.
pub fn eligible(text: &str, back: usize) -> bool {
    back == 0 && text.chars().count() <= MAX_UNDO_CHARS && !text.contains(['\n', '\r'])
}

/// Takes the expansion back, on a worker. Input: the app, the state and what to erase and restore. Returns how it ended: delivered without doing anything when the expander was stopped or its triggers swapped meanwhile (asked of the state after the wait and again, through `Hooks::proceed`, right before the keys go out; a clipboard that could not be given back is reported), failed after a failed put (which is reported), so `release` does not replay keys typed meanwhile.
pub fn run(app: &AppHandle, shared: &Shared, undo: &Undo) -> Outcome<Undo> {
    std::thread::sleep(AFTER_USERS_KEY);
    let current = lock(shared).flow.current();
    if !current {
        return Outcome::Delivered(None);
    }
    let (proceed, vetoed) = still_current(shared);
    let hooks = Hooks { proceed: Some(proceed), ..Default::default() };
    match put(undo.remove.saturating_sub(1), &undo.restore, 0, undo.insert, undo.settle, &undo.token, hooks) {
        Ok(()) => Outcome::Delivered(None),
        Err(e) if vetoed.load(Ordering::SeqCst) || e.text_went_in() => {
            status::report(app, &e.to_string());
            Outcome::Delivered(None)
        }
        Err(e) => {
            status::report(app, &e.to_string());
            Outcome::Failed
        }
    }
}
