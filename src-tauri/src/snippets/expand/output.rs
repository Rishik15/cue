//! Purpose: Puts an expansion into the app: replaces the trigger the user typed with the text, by typing, pasting, typing slowly or as formatted text, and moves the caret for a `{cursor}`.
//! Contents: put — the replacement; type_text — one insertion for the modes that erase separately (slow, formatted).

use super::super::model::Insert;
use super::richtext;
use crate::platform::{self, InsertOptions, Key};

fn type_text(text: &str, insert: Insert) -> Result<(), String> {
    match insert {
        // The trigger is already erased, so a failed typing run must not lose the expansion.
        Insert::Slow if !text.contains(['\n', '\r']) => platform::insert_slowly(text).map_err(|e| match platform::copy_text(text) {
            Ok(()) => format!("{e} The text is on your clipboard."),
            Err(_) => e,
        }),
        Insert::Rich => {
            platform::insert_rich(&richtext::to_plain(text), &richtext::to_html(text), &InsertOptions::default()).map_err(|e| e.to_string())
        }
        _ => platform::insert(text, &InsertOptions { always_paste: insert == Insert::Paste, restore_clipboard: true })
            .map_err(|e| e.to_string()),
    }
}

/// Replaces the trigger the user typed (`erase` characters) with `text`, then moves the caret `back` characters left. Text that is typed goes in together with the erasing as one batch of input, so a key the user types
/// meanwhile cannot land between the two.
pub fn put(erase: usize, text: &str, back: usize, insert: Insert) -> Result<(), String> {
    if matches!(insert, Insert::Auto | Insert::Paste) {
        let options = InsertOptions { always_paste: insert == Insert::Paste, restore_clipboard: true };
        return platform::replace(erase, text, back, &options).map_err(|e| e.to_string());
    }
    platform::press(Key::Backspace, erase).map_err(|e| format!("Could not erase the trigger: {e}"))?;
    type_text(text, insert)?;
    if back > 0 {
        let _ = platform::press(Key::Left, back);
    }
    Ok(())
}
