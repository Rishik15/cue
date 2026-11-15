//! File: strip/choose_tests.rs
//! Purpose: Protect the strip's chip choice: each detector fires on text that has its signal and stays quiet on text that does not; the chips follow the score (saved actions first, the last action used only breaking ties, never more than three, never twice, three whenever three apply, the naming cases only for a short single line); and no detector panics or reads past `SCAN` on arbitrary text (property tests).
//! Contents: detector cases, chooser cases and the property tests.

use proptest::prelude::*;

use super::choose::{chips, sample, score, signals, Candidate, Signals, CHIPS, SCAN};

/// Candidates from (id, saved) pairs.
fn rows<'a>(list: &[(&'a str, bool)]) -> Vec<Candidate<'a>> {
    list.iter().map(|(id, saved)| Candidate { id, saved: *saved }).collect()
}

/// The signals of a text, to read one field in a test.
fn sig(text: &str) -> Signals {
    signals(text)
}

/// JSON is seen by its opening bracket and a quoted key; prose with braces is not JSON.
#[test]
fn json_detector() {
    assert!(sig(r#"{"name": "cue", "on": true}"#).json);
    assert!(sig(" [ {\"a\":1} ]").json);
    assert!(!sig("{ not json at all }").json);
}

/// A link is seen, and a tracking parameter on a link is seen apart from it.
#[test]
fn link_detectors() {
    let g = sig("see https://example.com/a?utm_source=x&id=2 now");
    assert!(g.links && g.tracking);
    let plain = sig("see https://example.com/a?id=2");
    assert!(plain.links && !plain.tracking);
    assert!(!sig("no links, just utm_ words").links);
}

/// Doubled spaces, tabs, trailing spaces and three blank lines are spacing signals; clean text is not.
#[test]
fn spacing_detector() {
    assert!(sig("two  spaces").spacing);
    assert!(sig("a\tb").spacing);
    assert!(sig("trailing \nline").spacing);
    assert!(sig("a\n\n\n\nb").spacing);
    assert!(!sig("one space.\n\nTwo lines.").spacing);
}

/// Line breaks inside sentences are hard wrapping; lines that end their sentences are not.
#[test]
fn wrapped_detector() {
    assert!(sig("This was copied from a\npdf where every line\nbreaks in the middle").wrapped);
    assert!(!sig("One line.\nAnother line.\nA third.").wrapped);
}

/// Spoken fillers are seen as whole words only.
#[test]
fn filler_detector() {
    assert!(sig("so um I think, uh, yes").fillers);
    assert!(!sig("umbrella under the hummock").fillers);
}

/// A word twice in a row is seen across case and punctuation; numbers and different words are not.
#[test]
fn repeat_detector() {
    assert!(sig("for the The report").repeats);
    assert!(!sig("1 1 is two ones, then other words").repeats);
}

/// Shouted and all-lower text are case signals; normal sentences are neither.
#[test]
fn case_detectors() {
    assert!(sig("PLEASE READ THIS NOW").shouting);
    assert!(!sig("Please read NASA's note").shouting);
    assert!(sig("all of this is lower case").flat);
    assert!(!sig("Normal Sentence here and there").flat);
}

/// Curly quotes are seen, and so is a mix of curly and straight.
#[test]
fn quote_detectors() {
    assert!(sig("he said \u{201C}yes\u{201D}").quotes);
    assert!(sig("\u{201C}yes\u{201D} and \"no\"").mixed_quotes);
    assert!(!sig("plain \"quotes\" only").quotes);
}

/// Bulleted or numbered lines are a list; a sentence that starts with a number is not.
#[test]
fn list_detector() {
    assert!(sig("Plan\n• first\n• second").listy);
    assert!(sig("1. one\n2. two").listy);
    assert!(!sig("2024 was a year.").listy);
}

/// A short code name is an identifier, a few short words are a name, a paragraph is neither.
#[test]
fn naming_detectors() {
    assert!(sig("userAccountId").identifier);
    assert!(sig("user_account_id").identifier);
    assert!(!sig("user account id").identifier);
    assert!(sig("user account id").name);
    assert!(!sig(&"word ".repeat(30)).name);
}

/// Typos and prose are model signals; one clean short sentence is neither.
#[test]
fn model_detectors() {
    assert!(sig("i think so").typos);
    assert!(sig("we recieved it").typos);
    assert!(sig("Done. then more").typos);
    assert!(sig("Wait , what").typos);
    assert!(!sig("All good here.").typos);
    assert!(sig("First sentence. Second sentence.").prose);
    assert!(!sig("Short note").prose);
    assert!(sig(&"word ".repeat(80)).long);
    assert!(!sig(&"word ".repeat(79)).long);
}

/// A signal raises only its own actions; an unknown id scores 0.
#[test]
fn scores_follow_the_signals() {
    let g = sig(r#"{"a": 1}"#);
    assert!(score("action:json_pretty", &g) > score("action:upper_case", &g));
    assert_eq!(score("action:nothing", &g), 0);
    assert_eq!(score("markdown", &Signals::default()), 35);
}

/// Saved actions come first, in list order, then the best scores.
#[test]
fn saved_actions_lead_then_the_score() {
    let r = rows(&[("rule:0:Mine", true), ("action:clean_whitespace", false), ("action:remove_repeats", false), ("rule:1:Other", true)]);
    assert_eq!(chips(&r, None, "the the  report"), vec![0, 3, 2]);
}

/// The strip is ordered by score, not by group or list order.
#[test]
fn the_best_match_comes_first() {
    let r = rows(&[("action:clean_whitespace", false), ("action:clean_links", false), ("action:json_pretty", false)]);
    assert_eq!(chips(&r, None, r#"{"url": "https://x.io/a"}"#), vec![2, 1, 0]);
}

/// The last action used breaks a tie but never beats a better score.
#[test]
fn the_last_action_breaks_ties() {
    let r = rows(&[
        ("action:sort_lines", false),
        ("action:split_bullets", false),
        ("action:remove_repeats", false),
        ("action:dedupe_lines", false),
    ]);
    assert_eq!(chips(&r, Some("action:split_bullets"), "a a\nb"), vec![2, 1, 0]);
    assert_eq!(chips(&r, Some("action:dedupe_lines"), "a\nb"), vec![3, 0, 1]);
}

/// Three chips whenever three rows apply, fewer when fewer do, never one twice.
#[test]
fn three_when_three_apply() {
    let r = rows(&[("rule:0:A", true), ("rule:1:B", true), ("rule:2:C", true), ("rule:3:D", true)]);
    assert_eq!(chips(&r, None, "x").len(), CHIPS);
    let plain = rows(&[("action:sort_lines", false), ("action:split_bullets", false), ("action:dedupe_lines", false)]);
    assert_eq!(chips(&plain, None, "nothing special\nhere").len(), 3);
    let few = rows(&[("action:extract_emails", false)]);
    assert_eq!(chips(&few, Some("action:gone"), "x"), vec![0]);
}

/// A naming case earns a chip on a short single line, not on a paragraph; a saved action always does.
#[test]
fn naming_cases_only_for_a_short_line() {
    let r = rows(&[("action:snake_case", false), ("action:clean_whitespace", false)]);
    assert_eq!(chips(&r, None, "userAccountId"), vec![0, 1]);
    assert_eq!(chips(&r, None, "Hello\nWorld"), vec![1]);
    assert_eq!(chips(&r, None, &"word ".repeat(30)), vec![1]);
}

/// The sample is cut at a character boundary at or under `SCAN` bytes.
#[test]
fn the_sample_respects_char_boundaries() {
    let text = "é".repeat(SCAN);
    let s = sample(&text);
    assert!(s.len() <= SCAN && s.len() >= SCAN - 1);
}

proptest! {
    /// No detector panics on any text, and what follows the first `SCAN` bytes never changes a signal: the scan is bounded.
    #[test]
    fn detectors_are_bounded(head in "\\PC{0,600}", tail in "\\PC{0,200}") {
        let mut text = head.clone();
        while text.len() < SCAN + 8 {
            text.push_str(if head.is_empty() { "x y " } else { &head });
        }
        let base = signals(&text);
        prop_assert_eq!(base, signals(&format!("{text}{tail}")));
    }

    /// Any text gives at most three distinct chips that are row indexes.
    #[test]
    fn chips_are_valid(text in "\\PC{0,300}", n in 0usize..8) {
        let ids = ["action:correct", "action:upper_case", "action:json_pretty", "markdown", "rule:0:A", "action:clean_links", "action:camel_case", "action:join_lines"];
        let r: Vec<Candidate> = ids.iter().take(n).map(|id| Candidate { id, saved: id.starts_with("rule:") }).collect();
        let out = chips(&r, Some("action:upper_case"), &text);
        prop_assert!(out.len() <= CHIPS && out.iter().all(|i| *i < n));
        let seen: std::collections::HashSet<_> = out.iter().collect();
        prop_assert_eq!(seen.len(), out.len());
    }
}
