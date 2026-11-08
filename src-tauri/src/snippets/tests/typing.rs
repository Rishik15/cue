//! File: snippets/tests/typing.rs
//! Purpose: Protect trigger detection while the user types: which keystrokes complete a trigger, what is erased and retyped, and when typing history is forgotten.
//! Contents: scenario tests that feed characters to the real `Tracker` and `Matcher`.

use super::helpers::effective;
use crate::platform::Typed;
use crate::snippets::expand::matcher::{Hit, Matcher, Tracker};
use crate::snippets::model::{Effective, Mode};

/// Types `text` one character at a time. Inputs: the tracker, the matcher and the text. Returns every hit, in order.
fn type_text(tracker: &mut Tracker, matcher: &Matcher, text: &str) -> Vec<Hit> {
    text.chars().filter_map(|c| tracker.feed(Typed::Char(c), matcher)).collect()
}

/// A tracker and the hits from typing `text` against these triggers. Inputs: the triggers and the text. Returns the hits.
fn hits(rules: &[Effective], text: &str) -> Vec<Hit> {
    type_text(&mut Tracker::default(), &Matcher::new(rules), text)
}

/// A trigger that starts with a symbol fires on its last character and erases exactly what was typed.
#[test]
fn a_symbol_trigger_fires_on_its_last_key() {
    let found = hits(&[effective(1, ":sig")], "hello :sig");
    assert_eq!(found.len(), 1);
    assert_eq!((found[0].id, found[0].erase, found[0].retype.as_str(), found[0].typed.as_str()), (1, 4, "", ":sig"));
    assert!(hits(&[effective(1, ":sig")], ":si").is_empty());
}

/// A trigger that starts with a letter waits for the space or punctuation that ends the word, erases it too and types it back.
#[test]
fn a_word_trigger_waits_for_the_word_to_end_and_retypes_the_ending() {
    let rules = [effective(1, "btw")];
    assert!(hits(&rules, "btw").is_empty());
    let spaced = hits(&rules, "btw ");
    assert_eq!((spaced[0].erase, spaced[0].retype.as_str()), (4, " "));
    let stopped = hits(&rules, "so btw.");
    assert_eq!((stopped[0].erase, stopped[0].retype.as_str()), (4, "."));
}

/// A word trigger must start a word: `ty` (thanks) does not fire at the end of `party`, unless the snippet allows it inside words.
#[test]
fn a_word_trigger_does_not_fire_inside_another_word_unless_allowed() {
    assert_eq!(hits(&[effective(1, "ty")], "ty ").len(), 1);
    assert!(hits(&[effective(1, "ty")], "party ").is_empty());
    let mut inside = effective(1, "ty");
    inside.inside_word = true;
    assert_eq!(hits(&[inside], "party ").len(), 1);
}

/// A trigger typed in capitals still fires and reports that its case changed, so the text can follow it; with match-case on, the wrong case does not fire.
#[test]
fn trigger_case_is_followed_or_enforced() {
    let rules = [effective(1, "btw")];
    let found = hits(&rules, "BTW ");
    assert!(found[0].changed_case && found[0].typed == "BTW");
    assert!(Matcher::new(&rules).follows_case(found[0].rule));
    let mut strict = effective(2, "BTW");
    strict.match_case = true;
    assert!(hits(&[strict.clone()], "btw ").is_empty());
    assert_eq!(hits(&[strict], "BTW ").len(), 1);
}

/// Backspace takes the last character back, and Enter or any reset forgets what was typed (in a chat box Enter has already sent the message).
#[test]
fn backspace_edits_the_typed_text_and_enter_or_reset_forgets_it() {
    let matcher = Matcher::new(&[effective(1, ":sig")]);
    let mut tracker = Tracker::default();
    type_text(&mut tracker, &matcher, ":sx");
    tracker.feed(Typed::Backspace, &matcher);
    assert_eq!(type_text(&mut tracker, &matcher, "ig").len(), 1, "':s' + 'ig' completes the trigger after a correction");
    type_text(&mut tracker, &matcher, ":si");
    tracker.feed(Typed::Char('\n'), &matcher);
    assert!(type_text(&mut tracker, &matcher, "g").is_empty());
    type_text(&mut tracker, &matcher, ":si");
    tracker.feed(Typed::Reset, &matcher);
    assert!(type_text(&mut tracker, &matcher, "g").is_empty());
}

/// A trigger with spaces inside expands after the whole phrase, and a trailing space in a trigger is kept and typed back.
#[test]
fn phrases_and_trailing_spaces_are_handled() {
    let phrase = hits(&[effective(1, "good morning")], "oh good morning ");
    assert_eq!((phrase[0].erase, phrase[0].retype.as_str(), phrase[0].typed.as_str()), (13, " ", "good morning"));
    let trailing = hits(&[effective(2, "i ")], "i ");
    assert_eq!((trailing[0].erase, trailing[0].retype.as_str(), trailing[0].typed.as_str()), (2, " ", "i"));
}

/// A regular-expression trigger fires when the typed text ends with a match and hands over its groups; a pattern that could match nothing is ignored.
#[test]
fn pattern_triggers_report_their_groups_and_empty_patterns_are_ignored() {
    let found = hits(&[effective(1, r"re:(\d+)px")], "width 12px");
    assert_eq!((found[0].erase, found[0].typed.as_str()), (4, "12px"));
    assert!(found[0].captures.contains(&("1".to_string(), "12".to_string())));
    assert!(Matcher::new(&[effective(2, "re:a*")]).is_empty(), "a pattern matching the empty text would fire on every key");
    assert!(Matcher::new(&[effective(3, "re:(unclosed")]).is_empty());
}

/// Snippets can be limited to programs or skip some: a skip list wins, whole words in program names count, and `code` is not `codex`.
#[test]
fn program_lists_decide_where_a_trigger_may_expand() {
    let mut only = effective(1, ":a");
    only.only_apps = vec!["slack".into()];
    let mut skip = effective(2, ":b");
    skip.skip_apps = vec!["code".into(), "WezTerm\twezterm-gui".into()];
    let matcher = Matcher::new(&[only, skip]);
    assert!(matcher.allows(0, "slack") && !matcher.allows(0, "teams"));
    assert!(!matcher.allows(1, "code") && !matcher.allows(1, "code-insiders") && !matcher.allows(1, "wezterm-gui"));
    assert!(matcher.allows(1, "codex") && matcher.allows(1, "notepad"));
    assert!(!matcher.allows(7, "slack"), "an unknown rule never expands");
}

/// A mode set on the snippet beats the automatic choice: an Immediate letter trigger fires without waiting, a Delimiter symbol trigger waits.
#[test]
fn an_explicit_mode_overrides_the_automatic_one() {
    let mut now = effective(1, "btw");
    now.mode = Mode::Immediate;
    assert_eq!(hits(&[now], "btw").len(), 1);
    let mut later = effective(2, ":sig");
    later.mode = Mode::Delimiter;
    assert!(hits(&[later.clone()], ":sig").is_empty());
    assert_eq!(hits(&[later], ":sig ").len(), 1);
}
