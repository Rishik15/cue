//! File: engine.rs
//! Purpose: Evaluate compiled rules against a text event, in order, and chain their actions. Pure: no OS, no files, so it is unit-tested.
//! Contents: CompiledRule — a rule with its conditions built and ordered; compile — validate a stored rule; run — apply every matching
//! rule to a text and report the result and which rules matched.

use super::condition::{self, Compiled};
use super::model::{Condition, Match, Output, Rule, Trigger};

pub struct CompiledRule {
    pub rule: Rule,
    conditions: Vec<Compiled>,
    /// Some condition compares lower-cased text, so `conditions_hold` makes it once.
    needs_lower: bool,
    apps: Vec<String>,
}

#[derive(Debug, PartialEq)]
pub struct Outcome {
    pub text: String,
    pub output: Output,
    /// Names of the rules that matched, in order, for the test panel and the run log.
    pub matched: Vec<String>,
}

/// Size limits for a rule, so a hand-edited or hostile `rules.json` (or a buggy page) cannot make Cue allocate or loop without bound.
const MAX_NAME: usize = 100;
const MAX_ACTIONS: usize = 20;
const MAX_CONDITIONS: usize = 30;
const MAX_APPS: usize = 30;
const MAX_TEXT: usize = 500;
/// Regex conditions are the costly ones (each is built once and run on every text), so a rule gets few.
const MAX_MATCHES: usize = 5;

/// Checks a rule against the size limits. Input: the rule. Returns Err with a message without the rule's name or text.
fn within_limits(rule: &Rule) -> Result<(), String> {
    let too_long = |s: &str| s.chars().count() > MAX_TEXT;
    let text_too_long = rule.conditions.iter().any(|c| match c {
        Condition::Contains { value } | Condition::StartsWith { value } | Condition::EndsWith { value } => too_long(value),
        Condition::Matches { pattern } => too_long(pattern),
        _ => false,
    });
    let patterns = rule.conditions.iter().filter(|c| matches!(c, Condition::Matches { .. })).count();
    if patterns > MAX_MATCHES
        || rule.name.chars().count() > MAX_NAME
        || rule.actions.len() > MAX_ACTIONS
        || rule.conditions.len() > MAX_CONDITIONS
        || rule.apps.len() > MAX_APPS
        || rule.apps.iter().any(|a| too_long(a))
        || text_too_long
    {
        return Err(format!(
            "A rule is too large: at most {MAX_ACTIONS} actions, {MAX_CONDITIONS} conditions and {MAX_TEXT} characters in a condition."
        ));
    }
    Ok(())
}

/// Validates a stored rule and builds it, cheapest conditions first. Input: the rule. Returns it, or Err with a message without the rule's name or text.
pub fn compile(rule: Rule) -> Result<CompiledRule, String> {
    within_limits(&rule)?;
    if rule.name.trim().is_empty() {
        return Err("Every rule needs a name.".into());
    }
    if rule.actions.is_empty() {
        return Err("A rule has no actions.".to_string());
    }
    let mut conditions = rule.conditions.iter().map(condition::compile).collect::<Result<Vec<_>, _>>()?;
    conditions.sort_by_key(condition::cost);
    let apps = rule.apps.iter().map(|a| normalize_app(a)).collect();
    let needs_lower = conditions.iter().any(condition::needs_lower);
    Ok(CompiledRule { rule, conditions, needs_lower, apps })
}

/// The form in which program names are compared. Input: a program name. Returns it lower-case without `.exe`.
pub fn normalize_app(name: &str) -> String {
    let name = name.trim().to_lowercase();
    name.strip_suffix(".exe").map(String::from).unwrap_or(name)
}

impl CompiledRule {
    /// Whether the rule applies. Inputs: the event's trigger, the text and a lazy lookup of the active program. Returns the verdict.
    pub fn matches(&self, trigger: Trigger, text: &str, app: &dyn Fn() -> String) -> bool {
        if !self.rule.enabled || self.rule.trigger != trigger {
            return false;
        }
        // The program is looked up only when a rule is limited to programs, and only after the cheaper checks passed.
        self.conditions_hold(text) && (self.apps.is_empty() || self.apps.contains(&app()))
    }

    /// Whether the conditions hold on a text. Input: the text. Returns the verdict by the rule's any/all/none mode.
    fn conditions_hold(&self, text: &str) -> bool {
        let lower = if self.needs_lower { text.to_lowercase() } else { String::new() };
        let mut results = self.conditions.iter().map(|c| condition::test(c, text, &lower));
        match self.rule.when {
            Match::All => results.all(|r| r),
            Match::Any => self.conditions.is_empty() || results.any(|r| r),
            Match::None => !results.any(|r| r),
        }
    }
}

/// Runs the rules for `trigger` over `text`. A rule that changes nothing still counts as matched (it may route the output).
pub fn run(rules: &[CompiledRule], trigger: Trigger, text: &str, app: &dyn Fn() -> String) -> Outcome {
    let mut out = Outcome { text: text.to_string(), output: Output::Insert, matched: Vec::new() };
    for rule in rules {
        if !rule.matches(trigger, &out.text, app) {
            continue;
        }
        for action in &rule.rule.actions {
            if let Some(next) = action.apply(&out.text) {
                out.text = next;
            }
        }
        out.output = rule.rule.output;
        out.matched.push(rule.rule.name.clone());
        if rule.rule.stop {
            break;
        }
    }
    out
}
