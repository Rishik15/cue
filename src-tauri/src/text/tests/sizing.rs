//! File: text/tests/sizing.rs
//! Purpose: Protect the rule that text of any length is handled: nothing overflows the model's context, nothing is lost or changed by cutting, and the user is told what was not processed.
//! Contents: a proptest that windows always cover the text exactly and fit; pipeline tests with a stand-in model whose context is small: overflow, one huge paragraph, a text just under and just over the budget, the editing limit and its notice, the whole-text head and its notice, cancel between windows.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use proptest::prelude::*;

use crate::actions::{task, Action};
use crate::text::backend::Backend;
use crate::text::budget::{self, MARGIN};
use crate::text::cancel::Cancel;
use crate::text::chunk::Seg;
use crate::text::engine::Fail;
use crate::text::pipeline::{Outcome, Pipeline};
use crate::text::window;

use super::scripted::{scripted, Scripted};

/// A scripted backend whose request budget for `action` is `tokens` of estimated input. Inputs: the action, the input room in tokens and the answering closure. Returns the backend.
fn sized<F: Fn(&str) -> Result<String, Fail>>(action: Action, tokens: usize, answer: F) -> Scripted<F> {
    let mut backend = scripted(answer);
    let prefix = backend.prefix_tokens(task(action).expect("model task"));
    backend.context = prefix + MARGIN + 2 * tokens;
    backend
}

/// A correction run of `text` with `tokens` of input room, the model echoing every request. Returns the outcome and the requests seen.
fn echo_run(text: &str, tokens: usize) -> (Outcome, Vec<String>) {
    let backend = sized(Action::Correct, tokens, |unit| Ok(unit.to_string()));
    let outcome = Pipeline::new(&backend, &Cancel::never()).run(task(Action::Correct).expect("model task"), text);
    (outcome, backend.seen.into_inner())
}

/// The text of an edit that finished, whatever the notice.
fn edited_text(outcome: Outcome) -> String {
    match outcome {
        Outcome::Edited { text, .. } => text,
        other => panic!("expected an edit, got {other:?}"),
    }
}

proptest! {
    /// Whatever the text and the limit, the windows and the gaps between them add up to the original exactly, and every window fits the limit.
    #[test]
    fn windows_cover_the_text_exactly_and_fit(text in "[ a-zA-Z0-9.,;:!?\\n\u{e9}\u{4e2d}]{0,300}", limit in 1usize..40) {
        let fits = |s: &str| s.chars().count() <= limit;
        let segs = window::refine(vec![Seg::Unit(&text)], &fits, &|_| false, &Cancel::never());
        let joined: String = segs.iter().map(|s| match s { Seg::Unit(t) | Seg::Keep(t) => *t }).collect();
        prop_assert_eq!(&joined, &text);
        for seg in &segs {
            if let Seg::Unit(window) = seg {
                prop_assert!(fits(window), "window of {} chars over the limit {limit}", window.chars().count());
            }
        }
    }

    /// The head is a fitting slice of the text that starts at its first letter-bearing piece, and the whole text when it fits.
    #[test]
    fn the_head_is_a_fitting_slice(text in "[ a-z.,\\n]{0,200}", limit in 1usize..40) {
        let fits = |s: &str| s.chars().count() <= limit;
        let head = window::head(&text, &fits, &Cancel::never());
        let at = text.find(head).expect("head is a slice of the text");
        prop_assert!(!text[..at].chars().any(char::is_alphabetic) || head.is_empty());
        prop_assert!(fits(head));
        prop_assert_eq!(head == text, fits(&text));
    }
}

/// A paragraph with no punctuation, many times the budget, is cut at words: every request fits and the echoed text comes back exactly.
#[test]
fn a_huge_paragraph_without_punctuation_is_cut_at_words() {
    let text = "alpha beta gamma delta ".repeat(200).trim_end().to_string();
    let (outcome, seen) = echo_run(&text, 100);
    assert!(seen.len() > 10, "cut into {} windows", seen.len());
    assert!(seen.iter().all(|s| budget::estimate(s) <= 100));
    assert_eq!(edited_text(outcome), text);
}

/// A run of characters with no space at all is cut inside the word as the last resort, and still nothing is lost.
#[test]
fn a_word_larger_than_the_budget_is_cut_by_characters() {
    let text = "a".repeat(2000);
    let (outcome, seen) = echo_run(&text, 50);
    assert!(seen.iter().all(|s| budget::estimate(s) <= 50));
    assert_eq!(seen.concat(), text);
    assert_eq!(edited_text(outcome), text);
}

/// A sentence that is too long for one request is split at clauses before words, and each window is sent alone.
#[test]
fn an_oversized_sentence_is_split_at_clauses() {
    let clause = "this clause has seven plain words in it";
    let text = format!("{clause}, {clause}, {clause}.");
    let (outcome, seen) = echo_run(&text, 15);
    assert_eq!(seen.len(), 3, "{seen:?}");
    assert!(seen[0].ends_with(','));
    assert_eq!(edited_text(outcome), text);
}

/// A text one estimated token under the budget is one request; the same text a few characters longer is two.
#[test]
fn a_text_just_under_the_budget_is_one_request_and_just_over_is_two() {
    let under = format!("{}word", "word ".repeat(17));
    let over = format!("{}word", "word ".repeat(18));
    assert_eq!(budget::estimate(&under), 30);
    assert_eq!(budget::estimate(&over), 32);
    assert_eq!(echo_run(&under, 30).1.len(), 1);
    let (outcome, seen) = echo_run(&over, 30);
    assert_eq!(seen.len(), 2);
    assert_eq!(edited_text(outcome), over);
}

/// A model with almost no room still gets requests that fit: the budget never goes below nothing, and the run ends instead of looping.
#[test]
fn a_context_smaller_than_the_prefix_still_terminates() {
    let mut backend = scripted(|unit| Ok(unit.to_string()));
    backend.context = 10;
    let outcome = Pipeline::new(&backend, &Cancel::never()).run(task(Action::Correct).expect("model task"), "one two three.");
    assert_eq!(edited_text(outcome), "one two three.");
}

/// Past the editing limit the rest stays as written, the first sentences are edited, and the notice says how far the edit reached.
#[test]
fn editing_stops_at_the_limit_and_says_so() {
    let sentence = "one two three four.";
    let text = [sentence; 4].join(" ");
    let backend = scripted(|unit| Ok(unit.replacen("one", "One", 1)));
    let outcome = Pipeline::new(&backend, &Cancel::never()).with_total_chars(50).run(task(Action::Correct).expect("model task"), &text);
    let first = "One two three four.";
    let expected = format!("{first} {first} {sentence} {sentence}");
    assert_eq!(outcome, Outcome::Edited { text: expected, notice: Some("Edited the first 40 of 79 characters.".into()) });
    assert_eq!(backend.seen.borrow().len(), 2);
}

/// Text under the limit is edited whole and carries no notice.
#[test]
fn a_short_text_has_no_notice() {
    let backend = scripted(|unit| Ok(unit.to_string()));
    let outcome = Pipeline::new(&backend, &Cancel::never()).run(task(Action::Correct).expect("model task"), "One two. Three four.");
    assert_eq!(outcome, Outcome::Edited { text: "One two. Three four.".into(), notice: None });
}

/// A whole-text task sends one request holding the start of the text, and the notice names how much was used.
#[test]
fn a_whole_text_task_uses_the_head_that_fits() {
    let text = "Meeting notes. ".repeat(400).trim_end().to_string();
    let backend = sized(Action::Summarize, 200, |_| Ok("Notes about a meeting.".into()));
    let outcome = Pipeline::new(&backend, &Cancel::never()).run(task(Action::Summarize).expect("model task"), &text);
    let seen = backend.seen.into_inner();
    assert_eq!(seen.len(), 1);
    assert!(text.starts_with(&seen[0]));
    assert!(budget::estimate(&seen[0]) <= 200);
    let used = seen[0].chars().count();
    let total = text.chars().count();
    let notice = format!("Used the first {used} of {total} characters; the rest was left out.");
    assert_eq!(outcome, Outcome::Edited { text: "Notes about a meeting.".into(), notice: Some(notice) });
}

/// A whole-text task that fits has no notice and sends the text as it is.
#[test]
fn a_whole_text_task_that_fits_has_no_notice() {
    let backend = scripted(|_| Ok("A meeting.".into()));
    let outcome = Pipeline::new(&backend, &Cancel::never())
        .run(task(Action::Summarize).expect("model task"), "Meeting notes for the week, with some words.");
    assert_eq!(outcome, Outcome::Edited { text: "A meeting.".into(), notice: None });
}

/// A cancel between the windows of one giant paragraph ends the edit at once: no further window is sent.
#[test]
fn cancel_between_windows_stops_the_edit() {
    let text = "alpha beta gamma delta ".repeat(100);
    let asked = Arc::new(AtomicUsize::new(0));
    let cancel = Cancel::new({
        let asked = Arc::clone(&asked);
        move || asked.load(Ordering::SeqCst) >= 2
    });
    let backend = sized(Action::Correct, 40, |unit| {
        asked.fetch_add(1, Ordering::SeqCst);
        Ok(unit.to_string())
    });
    let outcome = Pipeline::new(&backend, &cancel).run(task(Action::Correct).expect("model task"), &text);
    assert_eq!(outcome, Outcome::Cancelled);
    assert_eq!(backend.seen.borrow().len(), 2);
}
