//! Purpose: Takes an expansion back when the user presses Backspace right after it (a setting, off by default): the text is erased and the trigger they typed is put back.
//! Contents: Undo — what to erase and what to restore; eligible — whether an expansion can be undone safely; run — the undo itself, on a worker.
//! Only short single-line expansions qualify: Backspace counts characters, and a pasted or multi-line text (or a caret moved by `{cursor}`) would not be erased exactly.

use std::time::Duration;

use tauri::AppHandle;

use super::super::model::Insert;
use super::type_text;
use crate::platform::{self, Key};
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
}

pub fn eligible(text: &str, back: usize) -> bool {
    back == 0 && text.chars().count() <= MAX_UNDO_CHARS && !text.contains(['\n', '\r'])
}

pub fn run(app: &AppHandle, undo: &Undo) {
    std::thread::sleep(AFTER_USERS_KEY);
    let done = platform::press(Key::Backspace, undo.remove.saturating_sub(1))
        .map_err(|e| format!("Could not take the expansion back: {e}"))
        .and_then(|()| type_text(&undo.restore, undo.insert));
    if let Err(e) = done {
        status::report(app, &e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_short_single_line_texts_with_the_caret_at_the_end_can_be_undone() {
        assert!(eligible("Best regards", 0));
        assert!(!eligible("two\nlines", 0));
        assert!(!eligible("caret inside", 3));
        assert!(!eligible(&"x".repeat(101), 0));
    }
}
