//! File: rules/tests/chain.rs
//! Purpose: Protect how saved actions run: in saved order on the current text, only for their trigger, conditions and programs, with stop and routing.
//! Contents: scenario tests over `engine::run` with compiled rules.

use std::cell::Cell;

use super::helpers::{rule, with_when};
use crate::actions::Action;
use crate::rules::engine::{self, compile, CompiledRule};
use crate::rules::model::{Condition, Kind, Match, Output, Trigger};

/// Compiles every rule, panicking on a rule the test wrote wrongly. Input: the rules. Returns them compiled.
fn built(rules: Vec<crate::rules::model::Rule>) -> Vec<CompiledRule> {
    rules.into_iter().map(|r| compile(r).expect("valid rule")).collect()
}

/// Dictation tidy-up, a Slack-only shout and a to-do rule run in saved order, each on what the one before made, and the last match decides where the text goes.
#[test]
fn matching_rules_chain_in_saved_order_and_the_last_match_routes_the_result() {
    let mut shout = rule("Shout in Slack", &[Action::UpperCase]);
    (shout.apps, shout.output) = (vec!["Slack.exe".into()], Output::Copy);
    let rules = built(vec![rule("Tidy", &[Action::CleanWhitespace, Action::RemoveFillers]), shout]);
    let slack = engine::run(&rules, Trigger::Dictation, "um   buy   milk", &|| "slack".into());
    assert_eq!((slack.text.as_str(), slack.output), ("BUY MILK", Output::Copy));
    assert_eq!(slack.matched, ["Tidy", "Shout in Slack"]);
    let browser = engine::run(&rules, Trigger::Dictation, "um   buy   milk", &|| "chrome".into());
    assert_eq!((browser.text.as_str(), browser.output), ("buy milk", Output::Insert));
    assert_eq!(browser.matched, ["Tidy"]);
}

/// Putting the rules the other way round gives a different text: each step sees the result of the one before, never the original.
#[test]
fn a_later_rule_sees_the_text_the_earlier_one_produced() {
    let join_then_bullets = built(vec![rule("Join", &[Action::JoinLines]), rule("Bullets", &[Action::SplitBullets])]);
    let bullets_then_join = built(vec![rule("Bullets", &[Action::SplitBullets]), rule("Join", &[Action::JoinLines])]);
    let run = |rules: &[CompiledRule]| engine::run(rules, Trigger::Dictation, "alpha\nbeta", &String::new).text;
    assert_eq!(run(&join_then_bullets), "- alpha beta");
    assert_eq!(run(&bullets_then_join), "- alpha\n- beta");
}

/// A rule marked stop ends the run, so later rules never touch the text.
#[test]
fn a_stop_rule_keeps_later_rules_from_running() {
    let mut todo = with_when(rule("To-do", &[Action::SplitBullets]), Match::All, vec![Condition::StartsWith { value: "todo".into() }]);
    (todo.stop, todo.output) = (true, Output::Preview);
    let rules = built(vec![todo, rule("Shout", &[Action::UpperCase])]);
    let out = engine::run(&rules, Trigger::Dictation, "Todo call mom. email dan", &String::new);
    assert_eq!(out.text, "- Todo call mom.\n- email dan");
    assert_eq!(out.output, Output::Preview);
    assert_eq!(out.matched, ["To-do"]);
}

/// A rule for selected text does not run on dictation, a disabled rule never runs, and a rule that changes nothing still counts as matched because it may route the output.
#[test]
fn triggers_and_the_enabled_switch_decide_eligibility_and_a_no_change_rule_still_routes() {
    let mut selection = rule("On selection", &[Action::UpperCase]);
    selection.trigger = Trigger::Selection;
    let mut off = rule("Off", &[Action::UpperCase]);
    off.enabled = false;
    let mut route = rule("Copy only", &[Action::CleanWhitespace]);
    route.output = Output::Copy;
    let rules = built(vec![selection, off, route]);
    let out = engine::run(&rules, Trigger::Dictation, "already tidy", &String::new);
    assert_eq!((out.text.as_str(), out.output), ("already tidy", Output::Copy));
    assert_eq!(out.matched, ["Copy only"]);
}

/// Any, All and None combine conditions; text conditions ignore case; a rule with no conditions always matches.
#[test]
fn condition_modes_combine_checks_and_ignore_letter_case() {
    let conds = || vec![Condition::Contains { value: "INVOICE".into() }, Condition::LooksLike { kind: Kind::Email }];
    let matches = |when: Match, text: &str| {
        let rules = built(vec![with_when(rule("R", &[Action::UpperCase]), when, conds())]);
        !engine::run(&rules, Trigger::Dictation, text, &String::new).matched.is_empty()
    };
    assert!(matches(Match::Any, "the invoice is late"));
    assert!(!matches(Match::All, "the invoice is late"));
    assert!(matches(Match::None, "hello there"));
    assert!(!matches(Match::None, "the invoice is late"));
    let empty = built(vec![with_when(rule("Always", &[Action::UpperCase]), Match::Any, vec![])]);
    assert_eq!(engine::run(&empty, Trigger::Dictation, "x", &String::new).text, "X");
}

/// The program in front is looked up only when a rule is limited to programs and the text already passed its cheaper conditions.
#[test]
fn the_foreground_program_is_looked_up_only_when_needed() {
    let mut scoped = with_when(rule("Scoped", &[Action::UpperCase]), Match::All, vec![Condition::Contains { value: "needle".into() }]);
    scoped.apps = vec!["notepad".into()];
    let rules = built(vec![rule("Plain", &[Action::LowerCase]), scoped]);
    let lookups = Cell::new(0);
    let lookup = || {
        lookups.set(lookups.get() + 1);
        "notepad".to_string()
    };
    engine::run(&rules, Trigger::Dictation, "no match here", &lookup);
    assert_eq!(lookups.get(), 0);
    let out = engine::run(&rules, Trigger::Dictation, "Find the Needle", &lookup);
    assert_eq!(out.text, "FIND THE NEEDLE");
    assert!(lookups.get() >= 1);
}

/// Length, regex and shape conditions behave as the Rules page describes them.
#[test]
fn length_regex_and_shape_conditions_match_what_they_say() {
    let check = |condition: Condition, text: &str| {
        let rules = built(vec![with_when(rule("R", &[Action::UpperCase]), Match::All, vec![condition])]);
        !engine::run(&rules, Trigger::Dictation, text, &String::new).matched.is_empty()
    };
    assert!(check(Condition::LongerThan { chars: 5 }, "abcdef"));
    assert!(!check(Condition::LongerThan { chars: 6 }, "abcdef"));
    assert!(check(Condition::ShorterThan { chars: 3 }, "ab"));
    assert!(check(Condition::Matches { pattern: r"^\d{3}-\d{4}$".into() }, "555-1234"));
    assert!(check(Condition::LooksLike { kind: Kind::Url }, " https://example.com/a "));
    assert!(!check(Condition::LooksLike { kind: Kind::Url }, "see https://example.com"));
    assert!(check(Condition::LooksLike { kind: Kind::Json }, "{\"a\": 1}"));
    assert!(!check(Condition::LooksLike { kind: Kind::Json }, "{oops"));
    assert!(check(Condition::LooksLike { kind: Kind::List }, "- a\n- b"));
    assert!(!check(Condition::LooksLike { kind: Kind::List }, "- only one"));
}
