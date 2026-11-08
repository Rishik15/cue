//! File: rules/tests/dry_run.rs
//! Purpose: Protect the Rules page test panel: it runs unsaved rules on sample text, leaves the text model out unless asked, and names why a rule cannot run.
//! Contents: tests of `dry_run`.

use super::helpers::rule;
use crate::actions::Action;
use crate::rules::dry_run;
use crate::rules::model::{Output, Trigger};

/// Without the model button, a model step is left out so typing in the editor never loads the model; the exact steps still run.
#[test]
fn the_test_panel_skips_model_steps_and_runs_the_exact_ones() {
    let rules = vec![rule("Fix", &[Action::Correct, Action::CleanWhitespace])];
    let out = dry_run(rules, Trigger::Dictation, "a   b", "", false).expect("valid");
    assert_eq!((out.text.as_str(), out.output), ("a b", Output::Insert));
    assert_eq!(out.matched, ["Fix"]);
}

/// A rule made only of model steps drops out of the dry run instead of failing with "has no actions"; a rule that was empty from the start still reports it.
#[test]
fn a_model_only_rule_drops_out_but_an_empty_rule_still_errors() {
    let out = dry_run(vec![rule("AI only", &[Action::Summarize])], Trigger::Dictation, "long text", "", false).expect("valid");
    assert!(out.matched.is_empty());
    assert_eq!(out.text, "long text");
    let Err(message) = dry_run(vec![rule("Empty", &[])], Trigger::Dictation, "x", "", false) else { panic!("an empty rule passed") };
    assert_eq!(message, "A rule has no actions.");
}

/// With the model button the model step stays; with no model installed it changes nothing yet the rule still counts as matched.
#[test]
fn running_with_the_model_keeps_its_steps() {
    let out = dry_run(vec![rule("AI only", &[Action::Summarize])], Trigger::Dictation, "long text", "", true).expect("valid");
    assert_eq!(out.matched, ["AI only"]);
    assert_eq!(out.text, "long text");
}

/// The pretend program is compared the way real ones are (any case, with or without .exe), and the trigger chosen in the panel decides which rules run.
#[test]
fn the_pretend_program_and_trigger_pick_the_rules() {
    let mut scoped = rule("Slack only", &[Action::UpperCase]);
    scoped.apps = vec!["slack".into()];
    let on = dry_run(vec![scoped.clone()], Trigger::Dictation, "hi", "SLACK.EXE", false).expect("valid");
    assert_eq!(on.text, "HI");
    let wrong_app = dry_run(vec![scoped.clone()], Trigger::Dictation, "hi", "teams", false).expect("valid");
    assert_eq!(wrong_app.text, "hi");
    let wrong_trigger = dry_run(vec![scoped], Trigger::Selection, "hi", "slack", false).expect("valid");
    assert!(wrong_trigger.matched.is_empty());
}
