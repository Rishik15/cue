//! File: text/tests/pipeline.rs
//! Purpose: Protect the model edit as a whole: each sentence or paragraph is sent alone, every reply is checked, and a refused reply leaves that unit exactly as written with all spacing intact.
//! Contents: scenario tests that run the real `split`, `accept` and `join` with a scripted stand-in for the model.

use crate::actions::{task, Action, Task, Unit};
use crate::sanitize::strip_hidden;
use crate::text::chunk::{join, split, Seg};
use crate::text::reply::accept;

/// Runs one edit the way `text::edit` does, with `model` answering each unit. Inputs: the task, the text and the stand-in model. Returns the edited text.
fn edit(task: &Task, text: &str, model: impl Fn(&str) -> String) -> String {
    let segs = split(text, task.unit);
    let answers: Vec<String> = segs
        .iter()
        .filter_map(|seg| match seg {
            Seg::Unit(unit) => {
                Some(accept(&strip_hidden(unit), &model(&strip_hidden(unit)), task.max_change).unwrap_or_else(|| unit.to_string()))
            }
            Seg::Keep(_) => None,
        })
        .collect();
    join(&segs, &answers)
}

/// A correction run over three sentences: a wrapped and quoted reply is tidied and used, a refusal and a reply that drops a link are refused, and the spacing between sentences is untouched.
#[test]
fn each_reply_is_judged_alone_and_refused_units_stay_as_written() {
    let correct = task(Action::Correct).expect("Correct uses the model");
    let text = "she dont like apples.  Call me at 555-1234!\n\nSee https://a.example/x?id=2 now.";
    let edited = edit(correct, text, |unit| match unit {
        "she dont like apples." => "Here is the corrected text:\n\"She doesn't like apples.\"".to_string(),
        "Call me at 555-1234!" => "I'm sorry, I can't help with that.".to_string(),
        _ => "See the link now.".to_string(),
    });
    assert_eq!(edited, "She doesn't like apples.  Call me at 555-1234!\n\nSee https://a.example/x?id=2 now.");
}

/// A model that only repeats its input changes nothing, whatever the layout: leading blanks, trailing line breaks and quotes after a full stop included.
#[test]
fn echoing_every_unit_reproduces_the_text_exactly() {
    let correct = task(Action::Correct).expect("model task");
    for text in ["  leading and trailing  \n", "He said \"go.\" Then left.", "one\ntwo\n\nthree", "Total 42. 17. Done.", ""] {
        assert_eq!(edit(correct, text, str::to_string), text);
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
    let correct = task(Action::Correct).expect("model task");
    let seen = std::cell::RefCell::new(Vec::new());
    let edited = edit(correct, "helo\u{200B} wor\u{E0041}ld.", |unit| {
        seen.borrow_mut().push(unit.to_string());
        "Hello\u{202E} world.".to_string()
    });
    assert_eq!(seen.into_inner(), ["helo world."]);
    assert_eq!(edited, "Hello world.");
}

/// With fewer replies than units the rest keep their original text, so a short answer list can never lose a sentence.
#[test]
fn missing_replies_leave_the_remaining_units_untouched() {
    let segs = split("First one. Second one. Third one.", Unit::Sentence);
    assert_eq!(join(&segs, &["1st.".to_string()]), "1st. Second one. Third one.");
}
