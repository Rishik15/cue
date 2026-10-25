//! Purpose: Puts an expansion into the app: replaces the trigger the user typed with the text, by typing, pasting, typing slowly or as formatted text, and moves the caret for a `{cursor}`.
//! Contents: put — the replacement. Every mode sends the erase of the trigger together with the text (one batch of input, or one lock and no pause between them), so a key the user types meanwhile cannot land between the two.

use super::super::model::Insert;
use super::richtext;
use crate::platform::{self, InsertOptions};

/// Replaces the trigger the user typed (`erase` characters) with `text`, then moves the caret `back` characters left.
pub fn put(erase: usize, text: &str, back: usize, insert: Insert) -> Result<(), String> {
    let options = InsertOptions { always_paste: insert == Insert::Paste, restore_clipboard: true };
    match insert {
        // The trigger is erased already when typing fails, so the expansion is not lost.
        Insert::Slow if !text.contains(['\n', '\r']) => {
            platform::insert_slowly(erase, text, back).map_err(|e| match platform::copy_text(text) {
                Ok(()) => format!("{e} The text is on your clipboard."),
                Err(_) => e,
            })
        }
        Insert::Rich => platform::replace_rich(erase, &richtext::to_plain(text), &richtext::to_html(text), back, &InsertOptions::default())
            .map_err(|e| e.to_string()),
        _ => platform::replace(erase, text, back, &options).map_err(|e| e.to_string()),
    }
}
