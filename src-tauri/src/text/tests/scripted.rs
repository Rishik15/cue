//! File: text/tests/scripted.rs
//! Purpose: The stand-in for the model that the pipeline tests share: it answers each unit from a closure, records what it was asked, and has a context size and a tokenizer the test can set.
//! Contents: Scripted — the `Backend`; scripted — build one with a 4096-token context; edited — the outcome of a finished edit without a notice.

use std::cell::RefCell;

use crate::actions::Task;
use crate::text::backend::Backend;
use crate::text::cancel::Cancel;
use crate::text::engine::Fail;
use crate::text::pipeline::Outcome;

/// A stand-in model: `answer` is asked each unit the pipeline sends, the units it saw are recorded, and `context` is the size it reports.
pub struct Scripted<F: Fn(&str) -> Result<String, Fail>> {
    pub answer: F,
    pub seen: RefCell<Vec<String>>,
    pub context: usize,
}

/// A scripted backend with a 4096-token context. Input: the answering closure. Returns the backend.
pub fn scripted<F: Fn(&str) -> Result<String, Fail>>(answer: F) -> Scripted<F> {
    Scripted { answer, seen: RefCell::new(Vec::new()), context: 4096 }
}

/// The outcome of an edit that finished with `text` and no notice.
pub fn edited(text: &str) -> Outcome {
    Outcome::Edited { text: text.to_string(), notice: None }
}

impl<F: Fn(&str) -> Result<String, Fail>> Backend for Scripted<F> {
    /// The scripted context size.
    fn context(&self) -> usize {
        self.context
    }

    /// Records the unit and returns the scripted answer.
    fn ask(&self, _task: &Task, text: &str, _max_tokens: usize, _cancel: &Cancel) -> Result<String, Fail> {
        self.seen.borrow_mut().push(text.to_string());
        (self.answer)(text)
    }
}
