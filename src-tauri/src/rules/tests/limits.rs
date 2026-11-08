//! File: rules/tests/limits.rs
//! Purpose: Protect the boundary where a hand-edited or hostile rules.json enters: bad rules are refused with a message that never quotes the user's pattern, and size caps hold.
//! Contents: tests of `compile` and `compile_all`.

use super::helpers::{rule, with_when};
use crate::actions::Action;
use crate::rules::compile_all;
use crate::rules::engine::compile;
use crate::rules::model::{Condition, Match, Rule};

/// The refusal message for a rule that must not compile. Input: the rule. Returns the message; panics when the rule was accepted.
fn refusal(rule: Rule) -> String {
    match compile(rule) {
        Ok(_) => panic!("the rule was accepted"),
        Err(message) => message,
    }
}

/// A rule needs a name and at least one step, and the message says so plainly.
#[test]
fn a_rule_without_a_name_or_steps_is_refused() {
    assert_eq!(refusal(rule("   ", &[Action::UpperCase])), "Every rule needs a name.");
    assert_eq!(refusal(rule("Empty", &[])), "A rule has no actions.");
}

/// A broken or runaway regex is refused, and the message never repeats the pattern (it might hold private text and ends up in the log).
#[test]
fn a_bad_or_too_complex_pattern_is_refused_without_quoting_it() {
    for pattern in ["(secret-unclosed", "(a{1000}){1000}"] {
        let message = refusal(with_when(rule("R", &[Action::UpperCase]), Match::All, vec![Condition::Matches { pattern: pattern.into() }]));
        assert!(message.contains("pattern"), "{message}");
        assert!(!message.contains("secret") && !message.contains("a{1000}"), "{message}");
    }
}

/// Size caps keep a hostile file from making Cue allocate or loop without bound: too many steps, conditions, regexes or too long a value.
#[test]
fn oversized_rules_are_refused_and_the_limits_are_exact() {
    let many_steps = vec![Action::UpperCase; 21];
    assert!(refusal(rule("Steps", &many_steps)).contains("too large"));
    assert!(compile(rule("Steps", &many_steps[..20])).is_ok());

    let regex = || Condition::Matches { pattern: "a".into() };
    let six_patterns = with_when(rule("Patterns", &[Action::UpperCase]), Match::All, (0..6).map(|_| regex()).collect());
    assert!(refusal(six_patterns).contains("too large"));
    let five_patterns = with_when(rule("Patterns", &[Action::UpperCase]), Match::All, (0..5).map(|_| regex()).collect());
    assert!(compile(five_patterns).is_ok());

    let long = with_when(rule("Long", &[Action::UpperCase]), Match::All, vec![Condition::Contains { value: "x".repeat(501) }]);
    assert!(refusal(long).contains("too large"));
}

/// The saved list is capped at 200 rules so a runaway page cannot fill memory.
#[test]
fn more_than_two_hundred_rules_are_refused() {
    let rules = |n: usize| (0..n).map(|i| rule(&format!("Rule {i}"), &[Action::UpperCase])).collect::<Vec<_>>();
    assert_eq!(compile_all(rules(200)).map(|r| r.len()), Ok(200));
    let Err(message) = compile_all(rules(201)) else { panic!("201 rules were accepted") };
    assert_eq!(message, "Too many rules (at most 200).");
}
