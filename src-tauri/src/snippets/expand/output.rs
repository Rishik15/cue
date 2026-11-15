//! File: output.rs
//! Purpose: Puts an expansion into the app: replaces the trigger the user typed with the text, by typing, pasting, typing slowly or as formatted text, and moves the caret for a `{cursor}`.
//! Contents: put — the replacement. Every mode sends the erase of the trigger together with the text (one batch of input, or one lock and no pause between them), so a key the user types meanwhile cannot land between the two.

use super::super::model::Insert;
use super::richtext;
use crate::platform::{self, InsertOptions, Target};

/// Replaces the trigger the user typed (`erase` characters) with `text`, then moves the caret `back` characters left, in every mode only if the keys still go to `token`, the window and field the trigger was typed in (taken when the trigger was seen, before any slow step): when focus moved meanwhile, nothing is erased or typed and the text goes to the clipboard. Slow typing erases the trigger first, so when it fails the expansion is copied to keep it.
pub fn put(erase: usize, text: &str, back: usize, insert: Insert, token: &Target) -> Result<(), String> {
    let options =
        InsertOptions { always_paste: insert == Insert::Paste, target: token.window, token: Some(token.clone()), ..Default::default() };
    match insert {
        Insert::Slow if !text.contains(['\n', '\r']) => {
            platform::insert_slowly(erase, text, back, token.clone()).map_err(|e| match platform::copy_text(text) {
                Ok(()) => format!("{e} The text is on your clipboard."),
                Err(_) => e,
            })
        }
        Insert::Rich => {
            platform::replace_rich(erase, &richtext::to_plain(text), &richtext::to_html(text), back, &options).map_err(|e| e.to_string())
        }
        _ => platform::replace(erase, text, back, &options).map_err(|e| e.to_string()),
    }
}
