//! File: text/tests/large.rs
//! Purpose: Protect the rule that input of any size costs a bounded amount of work: the model's tokenizer, the splitting and the requests only ever see a prefix of the text, and the text still comes back exactly.
//! Contents: a backend that counts tokenizer calls and the characters sent to it; tests of a whole-text task, an editing run over sentences, one giant unpunctuated word, and a cancel inside the splitting.

use std::cell::Cell;
use std::time::{Duration, Instant};

use crate::actions::{task, Action, Task};
use crate::text::backend::{Backend, Reply};
use crate::text::budget;
use crate::text::cancel::Cancel;
use crate::text::engine::Fail;
use crate::text::pipeline::{Outcome, Pipeline};

/// Most characters the tokenizer may be given over a whole run, whatever the input size.
const COUNTED_LIMIT: usize = 200_000;

/// Longest a run over megabytes of text may take.
const TIME_LIMIT: Duration = Duration::from_secs(5);

/// A stand-in model that echoes every request and counts what the pipeline asks the tokenizer: the calls and the characters.
struct Counting {
    calls: Cell<usize>,
    chars: Cell<usize>,
}

impl Counting {
    /// A backend that has counted nothing yet.
    fn new() -> Self {
        Self { calls: Cell::new(0), chars: Cell::new(0) }
    }
}

impl Backend for Counting {
    /// The usual 4096-token context.
    fn context(&self) -> usize {
        4096
    }

    /// Counts like the estimate and records the call.
    fn count(&self, text: &str) -> Option<usize> {
        self.calls.set(self.calls.get() + 1);
        self.chars.set(self.chars.get() + text.chars().count());
        Some(budget::estimate(text))
    }

    /// Echoes the unit, or answers a fixed summary for a whole-text task.
    fn ask(&self, task: &Task, text: &str, _max_tokens: usize, _cancel: &Cancel) -> Result<Reply, Fail> {
        let text = if task.system.contains("ummar") { "A summary.".to_string() } else { text.to_string() };
        Ok(Reply { text, cut: false })
    }
}

/// Runs `action` over `text` with the counting backend. Returns the outcome, the backend and the time the run took.
fn run(action: Action, text: &str, cancel: &Cancel) -> (Outcome, Counting, Duration) {
    let backend = Counting::new();
    let started = Instant::now();
    let outcome = Pipeline::new(&backend, cancel).run(task(action).expect("model task"), text);
    (outcome, backend, started.elapsed())
}

/// Asserts the work stayed bounded. Input: the backend that counted and the time taken.
fn assert_bounded(backend: &Counting, took: Duration) {
    assert!(backend.chars.get() <= COUNTED_LIMIT, "tokenizer saw {} characters", backend.chars.get());
    assert!(backend.calls.get() <= 200, "{} tokenizer calls", backend.calls.get());
    assert!(took < TIME_LIMIT, "took {took:?}");
}

/// A summary of megabytes of text tokenizes and splits only the start of it, and says how much was used.
#[test]
fn a_whole_text_task_on_megabytes_only_looks_at_the_start() {
    let text = "Meeting notes. ".repeat(350_000);
    let (outcome, backend, took) = run(Action::Summarize, &text, &Cancel::never());
    assert_bounded(&backend, took);
    let Outcome::Edited { text: answer, notice: Some(notice) } = outcome else { panic!("expected a summary with a notice") };
    assert_eq!(answer, "A summary.");
    assert!(notice.starts_with("Used the first "), "{notice}");
}

/// An editing run over megabytes of sentences splits only what it can reach, and the untouched rest comes back exactly.
#[test]
fn an_edit_of_megabytes_splits_only_what_it_can_reach() {
    let text = "One two three. ".repeat(300_000);
    let (outcome, backend, took) = run(Action::Correct, &text, &Cancel::never());
    assert_bounded(&backend, took);
    let Outcome::Edited { text: out, notice: Some(notice) } = outcome else { panic!("expected an edit with a notice") };
    assert_eq!(out, text);
    assert!(notice.starts_with("Edited the first "), "{notice}");
}

/// One giant word with no space or stop is not cut into one piece per character beyond what the run can reach.
#[test]
fn one_giant_word_is_not_cut_beyond_the_reach() {
    let text = "a".repeat(3_000_000);
    let (outcome, backend, took) = run(Action::Correct, &text, &Cancel::never());
    assert_bounded(&backend, took);
    let Outcome::Edited { text: out, .. } = outcome else { panic!("expected an edit") };
    assert_eq!(out, text);
}

/// A cancel set before the run ends it in the splitting, before any request.
#[test]
fn a_cancel_stops_the_splitting_of_a_huge_text() {
    let text = "a".repeat(3_000_000);
    let (outcome, backend, took) = run(Action::Correct, &text, &Cancel::new(|| true));
    assert_eq!(outcome, Outcome::Cancelled);
    assert_bounded(&backend, took);
}
