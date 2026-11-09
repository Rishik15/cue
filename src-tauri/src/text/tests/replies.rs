//! File: text/tests/replies.rs
//! Purpose: Protect the rule that model replies are untrusted: they are checked before they replace anything.
//! Contents: tests of `accept` (refusals, runaway answers, lost links and numbers, over-editing, wrapping) and the generation limit.

use crate::text::reply::{accept, bounded_distance, max_tokens, Reason, Verdict};
use proptest::prelude::*;

/// The answer text of an accepted verdict, or None for a rejection.
fn accepted(input: &str, answer: &str, max_change: Option<f32>) -> Option<String> {
    accept(input, answer, max_change).text().map(str::to_string)
}

/// Characters of `s`, lower-cased.
fn chars(s: &str) -> Vec<char> {
    s.to_lowercase().chars().collect()
}

/// Full edit distance, quadratic, as the reference for the banded one.
fn full_distance(a: &[char], b: &[char]) -> usize {
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            cur.push((prev[j] + usize::from(ca != cb)).min(prev[j + 1] + 1).min(cur[j] + 1));
        }
        prev = cur;
    }
    prev[b.len()]
}

/// A reply that obeys or answers the text instead of editing it changes far more than a correction may, so it is refused.
#[test]
fn a_correction_that_answers_the_text_instead_is_refused() {
    let input = "what is the capital of france";
    assert_eq!(accepted(input, "The capital of France is Paris.", Some(0.3)), None);
    assert_eq!(accepted(input, "What is the capital of France?", Some(0.3)).as_deref(), Some("What is the capital of France?"));
}

/// Refusals ("I'm sorry"), empty replies and runaway replies are never used, unless the user's own text began that way.
#[test]
fn refusals_empty_and_runaway_replies_are_rejected() {
    assert_eq!(accepted("fix this", "I'm sorry, I can't help with that.", None), None);
    assert_eq!(accepted("fix this", "   ", None), None);
    assert_eq!(accepted("ok", &"x".repeat(300), None), None);
    assert!(accepted("sorry, I am late", "Sorry, I am late.", None).is_some());
}

/// Links, emails, numbers, mentions and code survive an edit or the reply is refused.
#[test]
fn a_reply_that_loses_a_link_number_or_mention_is_refused() {
    let input = "Pay 1,200.50 by 5/3 to bob@acme.com, see https://a.example/x?id=2 @sam `run it`";
    assert!(accepted(input, "Pay 1,200.50 by 5/3 to bob@acme.com, see https://a.example/x?id=2 @sam `run it`.", None).is_some());
    for dropped in [
        "Pay 1200 by 5/3 to bob@acme.com, see https://a.example/x?id=2 @sam `run it`",
        "Pay 1,200.50 by 5/3, see https://a.example/x?id=2 @sam `run it`",
    ] {
        assert_eq!(accepted(input, dropped, None), None, "{dropped}");
    }
}

/// Wrapping the model adds is removed: a code fence, "Here is..." lead-ins and quotes, but quotes the input itself had stay.
#[test]
fn fences_lead_ins_and_added_quotes_are_stripped() {
    assert_eq!(accepted("fixd text", "```\nFixed text\n```", None).as_deref(), Some("Fixed text"));
    assert_eq!(accepted("fixd text", "Sure, here's the fixed text:\n\"Fixed text\"", None).as_deref(), Some("Fixed text"));
    assert_eq!(accepted("\"hello there\"", "\"Hello there\"", None).as_deref(), Some("\"Hello there\""));
}

/// The generation limit grows with the input and never passes 2048 tokens; edit distance ignores case.
#[test]
fn generation_is_bounded_and_edit_distance_ignores_case() {
    assert_eq!(max_tokens(""), 80);
    assert_eq!(max_tokens(&"a".repeat(10_000)), 2048);
    assert_eq!(bounded_distance(&chars("kitten"), &chars("sitting"), 10), 3);
    assert_eq!(bounded_distance(&chars("ABC"), &chars("abc"), 10), 0);
}

/// Protected tokens are compared whole: a changed amount, a link with a path added and a dropped duplicate are all refused, with the reason.
#[test]
fn protected_tokens_are_compared_whole_as_a_multiset() {
    let lost = Verdict::Reject(Reason::LostProtected);
    assert_eq!(accept("Pay 10 now", "Pay 100 now", None), lost);
    assert_eq!(accept("See https://a.example now", "See https://a.example/extra now", None), lost);
    assert_eq!(accept("Send 5 and 5 now", "Send 5 now", None), lost);
    assert_eq!(accept("Send 5 and 5 now", "Send 5 and 5 now.", None).text(), Some("Send 5 and 5 now."));
}

/// Each refusal carries its own reason code.
#[test]
fn rejections_name_their_reason() {
    assert_eq!(accept("fix this", " ", None), Verdict::Reject(Reason::Empty));
    assert_eq!(accept("fix this", "I cannot do that", None), Verdict::Reject(Reason::Refused));
    assert_eq!(accept("ok", &"x".repeat(300), None), Verdict::Reject(Reason::Runaway));
    assert_eq!(accept("what is the capital of france", "The capital of France is Paris.", Some(0.3)), Verdict::Reject(Reason::OverEdited));
    assert_eq!(Reason::LostProtected.code(), "lost-protected");
}

/// Identical long input and output, and a long input with a few edits, are judged without the quadratic table.
#[test]
fn long_texts_are_checked_in_linear_time() {
    let text = "word ".repeat(4000);
    let started = std::time::Instant::now();
    assert_eq!(accept(&text, &text, Some(0.3)).text(), Some(text.trim()));
    let edited = text.replacen("word", "ward", 3);
    assert!(accept(&text, &edited, Some(0.3)).text().is_some());
    assert!(started.elapsed() < std::time::Duration::from_secs(1));
}

/// A link wrapped in quotes or angle brackets, or a code span in brackets, still counts as the same link; a changed amount is still refused.
#[test]
fn wrapped_tokens_match_and_changed_amounts_do_not() {
    let input = "Open https://x.com/a now";
    for wrapped in [
        "Open \"https://x.com/a\" now",
        "Open <https://x.com/a> now",
        "Open (https://x.com/a) now",
        "Open [https://x.com/a] now",
        "Open \u{201c}https://x.com/a\u{201d} now",
    ] {
        assert!(accepted(input, wrapped, None).is_some(), "{wrapped}");
    }
    assert_eq!(accept("Pay 10 now", "Pay \"100\" now", None), Verdict::Reject(Reason::LostProtected));
    assert_eq!(accept("Pay 10 now", "Pay 100 now", None), Verdict::Reject(Reason::LostProtected));
}

proptest! {
    /// The banded distance equals the full distance when that is within the limit, and limit + 1 otherwise.
    #[test]
    fn banded_distance_matches_the_full_distance_capped(a in "[a-c]{0,24}", b in "[a-c]{0,24}", limit in 0usize..12) {
        let (a, b) = (chars(&a), chars(&b));
        prop_assert_eq!(bounded_distance(&a, &b, limit), full_distance(&a, &b).min(limit + 1));
    }
}
