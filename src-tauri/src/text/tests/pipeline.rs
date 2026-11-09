//! File: text/tests/pipeline.rs
//! Purpose: Protect the model edit as a whole: each sentence or paragraph is sent alone, every reply is checked, and a refused reply leaves that unit exactly as written with all spacing intact.
//! Contents: scenario tests that run the real `split`, `accept` and `join` with a scripted stand-in for the model.

use crate::actions::{task, Action, Unit};
use crate::text::cancel::Cancel;
use crate::text::chunk::{join, split, Seg};
use crate::text::engine::Fail;
use crate::text::pipeline::{Outcome, Pipeline};
use crate::text::reply::Reason;

use super::scripted::{edited, scripted};

/// Runs the pipeline for `action` over `text` with `model` answering each unit. Returns the outcome and the units the model was asked.
fn edit_with(action: Action, text: &str, cancel: &Cancel, model: impl Fn(&str) -> Result<String, Fail>) -> (Outcome, Vec<String>) {
    let backend = scripted(model);
    let outcome = Pipeline::new(&backend, cancel).run(task(action).expect("model task"), text);
    (outcome, backend.seen.into_inner())
}

/// Runs one correction edit that never cancels and never fails. Returns the edited text, or the text itself when no answer was usable.
fn edit(text: &str, model: impl Fn(&str) -> String) -> String {
    match edit_with(Action::Correct, text, &Cancel::never(), |unit| Ok(model(unit))).0 {
        Outcome::Edited { text: out, .. } => out,
        _ => text.to_string(),
    }
}

/// A correction run over three sentences: a wrapped and quoted reply is tidied and used, a refusal and a reply that drops a link are refused, and the spacing between sentences is untouched.
#[test]
fn each_reply_is_judged_alone_and_refused_units_stay_as_written() {
    let text = "she dont like apples.  Call me at 555-1234!\n\nSee https://a.example/x?id=2 now.";
    let edited = edit(text, |unit| match unit {
        "she dont like apples." => "Here is the corrected text:\n\"She doesn't like apples.\"".to_string(),
        "Call me at 555-1234!" => "I'm sorry, I can't help with that.".to_string(),
        _ => "See the link now.".to_string(),
    });
    assert_eq!(edited, "She doesn't like apples.  Call me at 555-1234!\n\nSee https://a.example/x?id=2 now.");
}

/// A model that only repeats its input changes nothing, whatever the layout: leading blanks, trailing line breaks and quotes after a full stop included.
#[test]
fn echoing_every_unit_reproduces_the_text_exactly() {
    for text in ["  leading and trailing  \n", "He said \"go.\" Then left.", "one\ntwo\n\nthree", "Total 42. 17. Done.", ""] {
        assert_eq!(edit(text, str::to_string), text);
    }
}

/// Paragraph tasks send whole paragraphs, not lines; units with no letters (numbers, symbols) are never sent at all.
#[test]
fn paragraphs_are_cut_at_blank_lines_and_letterless_pieces_are_kept() {
    let segs = split("First line one\nline two\n\nSecond para", Unit::Paragraph);
    assert_eq!(segs, [Seg::Unit("First line one\nline two"), Seg::Keep("\n\n"), Seg::Unit("Second para")]);
    let sentences = split("Total 42. 17. Done.", Unit::Sentence);
    assert_eq!(sentences, [Seg::Unit("Total 42."), Seg::Keep(" "), Seg::Keep("17."), Seg::Keep(" "), Seg::Unit("Done.")]);
}

/// A reply with hidden characters is cleaned before it can replace anything, and the unit sent to the model never carried any.
#[test]
fn hidden_characters_never_travel_to_or_from_the_model() {
    let (outcome, seen) =
        edit_with(Action::Correct, "helo\u{200B} wor\u{E0041}ld.", &Cancel::never(), |_| Ok("Hello\u{202E} world.".into()));
    assert_eq!(seen, ["helo world."]);
    assert_eq!(outcome, edited("Hello world."));
}

/// With fewer replies than units the rest keep their original text, so a short answer list can never lose a sentence.
#[test]
fn missing_replies_leave_the_remaining_units_untouched() {
    let segs = split("First one. Second one. Third one.", Unit::Sentence);
    assert_eq!(join(&segs, &["1st.".to_string()]), "1st. Second one. Third one.");
}

/// Some answers kept and some refused: the good ones are used, the refused unit stays as written, and the edit still counts as done.
#[test]
fn a_partly_usable_edit_keeps_the_good_answers() {
    let (outcome, _) = edit_with(Action::Correct, "she dont care. Call 555-1234!", &Cancel::never(), |unit| {
        Ok(if unit.starts_with("she") { "She doesn't care.".into() } else { "Sorry, I can't.".into() })
    });
    assert_eq!(outcome, edited("She doesn't care. Call 555-1234!"));
}

/// When every answer is refused the outcome says so, instead of reporting an edit that changed nothing.
#[test]
fn an_edit_where_every_answer_is_refused_has_no_usable_answer() {
    let (outcome, seen) = edit_with(Action::Correct, "one thing. two things.", &Cancel::never(), |_| Ok("I cannot do that".into()));
    assert_eq!(outcome, Outcome::NoUsableAnswer(Reason::Refused));
    assert_eq!(seen.len(), 2);
}

/// A text with no letters sends nothing to the model and comes back as it was.
#[test]
fn a_text_with_nothing_to_edit_is_returned_as_it_was() {
    let (outcome, seen) = edit_with(Action::Correct, "12 34", &Cancel::never(), |_| Err(Fail::Failed("never asked".into())));
    assert_eq!(outcome, edited("12 34"));
    assert!(seen.is_empty());
}

/// A backend failure ends the edit with its message; a cancel before or between units ends it with Cancelled and sends no more units.
#[test]
fn failures_and_cancels_end_the_edit() {
    let (failed, _) = edit_with(Action::Correct, "one thing.", &Cancel::never(), |_| Err(Fail::Failed("engine down".into())));
    assert_eq!(failed, Outcome::Failed("engine down".into()));
    let (already, seen) = edit_with(Action::Correct, "one thing.", &Cancel::new(|| true), |_| Ok("x".into()));
    assert_eq!((already, seen.len()), (Outcome::Cancelled, 0));
    let (backend_cancel, _) = edit_with(Action::Correct, "one thing.", &Cancel::never(), |_| Err(Fail::Cancelled));
    assert_eq!(backend_cancel, Outcome::Cancelled);
    let asked = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let cancel = Cancel::new({
        let asked = std::sync::Arc::clone(&asked);
        move || asked.load(std::sync::atomic::Ordering::SeqCst) >= 1
    });
    let (between, seen) = edit_with(Action::Correct, "one thing. two things. three things.", &cancel, |unit| {
        asked.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(unit.to_string())
    });
    assert_eq!((between, seen.len()), (Outcome::Cancelled, 1));
}
