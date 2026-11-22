//! File: output.rs
//! Purpose: Puts an expansion into the app: replaces the trigger the user typed with the text, by pasting it, plain or as formatted text, and moves the caret for a `{cursor}`.
//! Contents: put — the replacement; Hooks — what runs at the edges of a delivery; Delivery / classify — how a finished put counts: clean, text in with a warning, or not delivered; Completion / complete — what a finished put means for the expansion: its outcome, whether the use is counted and what is reported. Every mode sends the erase of the trigger together with the text (one batch of input, or one lock and no pause between them), so a key the user types meanwhile cannot land between the two.

use std::time::Duration;

use super::super::model::Insert;
use super::flow::Outcome;
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

/// What the caller hooks into a delivery.
#[derive(Default)]
pub struct Hooks {
    /// Runs the moment the text is sent (see `InsertOptions::on_sent`).
    pub on_sent: Option<Box<dyn Fn() + Send + Sync>>,
    /// Asked right before the keys go out; false cancels the delivery (see `InsertOptions::proceed`).
    pub proceed: Option<Box<dyn Fn() -> bool + Send + Sync>>,
}

/// What a finished `put` means for the expansion.
pub struct Completion<U> {
    /// How the worker ends: delivered (with the undo, only after a clean put) or failed.
    pub outcome: Outcome<U>,
    /// Whether the snippet's use counts: the text went in, with or without a warning.
    pub counted: bool,
    /// What to tell the user, if anything.
    pub report: Option<String>,
}

/// The completion policy: a clean put counts and may arm an undo, a warned put counts but arms none, a failed put counts nothing and fails the expansion. Input: the result of `put` and what arms an undo (asked for a clean put only). Returns the `Completion`.
pub fn complete<U>(sent: Result<(), InsertError>, arm: impl FnOnce() -> Option<U>) -> Completion<U> {
    match classify(sent) {
        Delivery::Clean => Completion { outcome: Outcome::Delivered(arm()), counted: true, report: None },
        Delivery::Warned(e) => Completion { outcome: Outcome::Delivered(None), counted: true, report: Some(e.to_string()) },
        Delivery::Failed(e) => Completion { outcome: Outcome::Failed, counted: false, report: Some(e.to_string()) },
    }
}

/// Replaces the trigger the user typed (`erase` characters) with `text`, then moves the caret `back` characters left, and runs `hooks.on_sent` the moment the text is sent (a paste waits `settle` longer for the target, the Response Time setting). In every mode the keys go out only if `hooks.proceed` still says yes just before them (a cancelled delivery erases and types nothing) and the keys still go to `token`, the window and field the trigger was typed in (taken when the trigger was seen): when focus moved meanwhile, nothing is erased or typed and the text goes to the clipboard.
pub fn put(
    erase: usize,
    text: &str,
    back: usize,
    insert: Insert,
    settle: Duration,
    token: &Target,
    hooks: Hooks,
) -> Result<(), InsertError> {
    let options = options(token, settle, hooks);
    match insert {
        Insert::Rich => platform::replace_rich(erase, &richtext::to_plain(text), &richtext::to_html(text), back, &options),
        Insert::Paste => platform::replace(erase, text, back, &options),
    }
}

/// The delivery options of one expansion. Input: the target the trigger was typed in, the paste settle time and the hooks. Returns the options with both hooks carried over.
pub fn options(token: &Target, settle: Duration, hooks: Hooks) -> InsertOptions {
    InsertOptions {
        always_paste: true,
        target: token.window,
        token: Some(token.clone()),
        settle,
        on_sent: hooks.on_sent,
        proceed: hooks.proceed,
        ..Default::default()
    }
}
