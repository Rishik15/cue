//! File: output.rs
//! Purpose: Puts an expansion into the app: replaces the trigger the user typed with the text, by pasting it, plain or as formatted text, and moves the caret for a `{cursor}`.
//! Contents: put — the replacement; Delivery / classify — how a finished put counts: clean, text in with a warning, or not delivered. Every mode sends the erase of the trigger together with the text (one batch of input, or one lock and no pause between them), so a key the user types meanwhile cannot land between the two.

use std::time::Duration;

use super::super::model::Insert;
use super::richtext;
use crate::platform::{self, InsertError, InsertOptions, Target};

/// How a finished `put` counts for stats, undo and the report.
pub enum Delivery {
    /// Everything asked for happened.
    Clean,
    /// The text went in but a side effect failed (clipboard not restored, caret not moved): it counts as sent, is reported, and arms no undo.
    Warned(InsertError),
    /// The text did not (certainly) go in: reported, never counted, never retried.
    Failed(InsertError),
}

/// Sorts the result of `put`. Input: that result. Returns its `Delivery`; only an error that says the text reached the app is a warning, every other (including an uncertain one) is a failure.
pub fn classify(result: Result<(), InsertError>) -> Delivery {
    match result {
        Ok(()) => Delivery::Clean,
        Err(e) if e.text_went_in() => Delivery::Warned(e),
        Err(e) => Delivery::Failed(e),
    }
}

/// Replaces the trigger the user typed (`erase` characters) with `text`, then moves the caret `back` characters left, and runs `on_sent` the moment the text is sent (a paste waits `settle` longer for the target, the Response Time setting), in every mode only if the keys still go to `token`, the window and field the trigger was typed in (taken when the trigger was seen): when focus moved meanwhile, nothing is erased or typed and the text goes to the clipboard.
pub fn put(
    erase: usize,
    text: &str,
    back: usize,
    insert: Insert,
    settle: Duration,
    token: &Target,
    on_sent: Option<Box<dyn Fn() + Send + Sync>>,
) -> Result<(), InsertError> {
    let options =
        InsertOptions { always_paste: true, target: token.window, token: Some(token.clone()), settle, on_sent, ..Default::default() };
    match insert {
        Insert::Rich => platform::replace_rich(erase, &richtext::to_plain(text), &richtext::to_html(text), back, &options),
        Insert::Paste => platform::replace(erase, text, back, &options),
    }
}
