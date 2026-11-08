//! File: rules/tests/helpers.rs
//! Purpose: Small builders so rule tests read as the rules a user would write.
//! Contents: rule — a rule with a name, trigger and steps; with_when — conditions and how they combine.

use crate::actions::Action;
use crate::rules::model::{Condition, Match, Output, Rule, Trigger};

/// A dictation rule with no conditions, inserting its result. Inputs: name and ordered steps. Returns the rule.
pub fn rule(name: &str, actions: &[Action]) -> Rule {
    Rule {
        name: name.into(),
        enabled: true,
        trigger: Trigger::Dictation,
        apps: vec![],
        when: Match::All,
        conditions: vec![],
        actions: actions.to_vec(),
        output: Output::Insert,
        stop: false,
    }
}

/// The rule with conditions that combine by `when`. Inputs: the rule, how conditions combine, the conditions. Returns the rule.
pub fn with_when(mut rule: Rule, when: Match, conditions: Vec<Condition>) -> Rule {
    rule.when = when;
    rule.conditions = conditions;
    rule
}
