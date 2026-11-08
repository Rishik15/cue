//! File: text/tests/replies.rs
//! Purpose: Protect the rule that model replies are untrusted: they are checked before they replace anything.
//! Contents: tests of `accept` (refusals, runaway answers, lost links and numbers, over-editing, wrapping) and the generation limit.

use crate::text::reply::{accept, changed_chars, max_tokens};

/// A reply that obeys or answers the text instead of editing it changes far more than a correction may, so it is refused.
#[test]
fn a_correction_that_answers_the_text_instead_is_refused() {
    let input = "what is the capital of france";
    assert_eq!(accept(input, "The capital of France is Paris.", Some(0.3)), None);
    assert_eq!(accept(input, "What is the capital of France?", Some(0.3)).as_deref(), Some("What is the capital of France?"));
}

/// Refusals ("I'm sorry"), empty replies and runaway replies are never used, unless the user's own text began that way.
#[test]
fn refusals_empty_and_runaway_replies_are_rejected() {
    assert_eq!(accept("fix this", "I'm sorry, I can't help with that.", None), None);
    assert_eq!(accept("fix this", "   ", None), None);
    assert_eq!(accept("ok", &"x".repeat(300), None), None);
    assert!(accept("sorry, I am late", "Sorry, I am late.", None).is_some());
}

/// Links, emails, numbers, mentions and code survive an edit or the reply is refused.
#[test]
fn a_reply_that_loses_a_link_number_or_mention_is_refused() {
    let input = "Pay 1,200.50 by 5/3 to bob@acme.com, see https://a.example/x?id=2 @sam `run it`";
    assert!(accept(input, "Pay 1,200.50 by 5/3 to bob@acme.com, see https://a.example/x?id=2 @sam `run it`.", None).is_some());
    for dropped in [
        "Pay 1200 by 5/3 to bob@acme.com, see https://a.example/x?id=2 @sam `run it`",
        "Pay 1,200.50 by 5/3, see https://a.example/x?id=2 @sam `run it`",
    ] {
        assert_eq!(accept(input, dropped, None), None, "{dropped}");
    }
}

/// Wrapping the model adds is removed: a code fence, "Here is..." lead-ins and quotes, but quotes the input itself had stay.
#[test]
fn fences_lead_ins_and_added_quotes_are_stripped() {
    assert_eq!(accept("fixd text", "```\nFixed text\n```", None).as_deref(), Some("Fixed text"));
    assert_eq!(accept("fixd text", "Sure, here's the fixed text:\n\"Fixed text\"", None).as_deref(), Some("Fixed text"));
    assert_eq!(accept("\"hello there\"", "\"Hello there\"", None).as_deref(), Some("\"Hello there\""));
}

/// The generation limit grows with the input and never passes 2048 tokens; edit distance ignores case.
#[test]
fn generation_is_bounded_and_edit_distance_ignores_case() {
    assert_eq!(max_tokens(""), 80);
    assert_eq!(max_tokens(&"a".repeat(10_000)), 2048);
    assert_eq!(changed_chars("kitten", "sitting"), 3);
    assert_eq!(changed_chars("ABC", "abc"), 0);
}
