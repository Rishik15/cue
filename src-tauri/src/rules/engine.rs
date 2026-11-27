//! File: engine.rs
//! Purpose: Evaluate compiled rules against a text event, in order, and chain their actions. Pure: no OS, no files, so it is unit-tested.
//! Contents: CompiledRule — a rule with its conditions built and ordered; compile — validate a stored rule; compile_all — validate and build a list; run — apply every matching
//! rule to a text and report the result and which rules matched; MAX_EVENT_BYTES — the longest text rules look at.

use super::condition::{self, Compiled};
use super::model::{Condition, Match, Output, Rule, Trigger};
use crate::actions::Step;

/// A rule with its conditions compiled and its program names normalised, ready to run.
pub struct CompiledRule {
    pub rule: Rule,
    conditions: Vec<Compiled>,
    /// Some condition compares lower-cased text, so `conditions_hold` makes it once.
    needs_lower: bool,
    apps: Vec<String>,
}

/// What a rule run produced: the final text, where it goes, and which rules matched.
#[derive(Debug, PartialEq)]
pub struct Outcome {
    pub text: String,
    pub output: Output,
    /// Names of the rules that matched, in order, for the test panel and the run log.
    pub matched: Vec<String>,
    /// The text was longer than `MAX_EVENT_BYTES` before a rule for the trigger ran, so that rule and the ones after it were skipped and `text` is what the earlier rules left (the original when none ran).
    pub skipped: bool,
}

/// Size limits for a rule, so a hand-edited or hostile `rules.json` (or a buggy page) cannot make Cue allocate or loop without bound.
const MAX_NAME: usize = 100;
const MAX_ACTIONS: usize = 20;
const MAX_CONDITIONS: usize = 30;
const MAX_APPS: usize = 30;
const MAX_TEXT: usize = 500;
/// Regex conditions are the costly ones (each is built once and run on every text), so a rule gets few.
const MAX_MATCHES: usize = 5;

/// More rules than anyone writes by hand; the cap keeps a runaway file or page from filling memory.
const MAX_RULES: usize = 200;
/// The longest text, in bytes, that rules look at (about 15,000 words, far past any dictation). Matching cost grows with the text (measured: 50 ms at 100 KB, 0.7 s at 1 MB, 9 s at 10 MB with 200 maximal rules), so a longer text skips every rule and is left as it is (`Outcome::skipped` says so when a rule was on).
pub const MAX_EVENT_BYTES: usize = 100_000;

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

/// Builds every stored rule. Input: the rules. Returns them compiled, or Err with a message that never quotes rule text.
pub fn compile_all(rules: Vec<Rule>) -> Result<Vec<CompiledRule>, String> {
    if rules.len() > MAX_RULES {
        return Err(format!("Too many rules (at most {MAX_RULES})."));
    }
    rules.into_iter().map(compile).collect()
}

/// The form in which program names are compared. Input: a program name. Returns it lower-case without `.exe`.
pub fn normalize_app(name: &str) -> String {
    let name = name.trim().to_lowercase();
    name.strip_suffix(".exe").map(String::from).unwrap_or(name)
}

impl CompiledRule {
    /// Whether the rule applies. Inputs: the event's trigger, the text and a lazy lookup of the active program. Returns the verdict; never for a text longer than `MAX_EVENT_BYTES`. The program is looked up only when the rule is limited to programs, and only after the cheaper checks passed.
    pub fn matches(&self, trigger: Trigger, text: &str, app: &dyn Fn() -> String) -> bool {
        if !self.rule.enabled || self.rule.trigger != trigger || text.len() > MAX_EVENT_BYTES {
            return false;
        }
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

/// The text after every action of `rule`, in order. Input: the rule and the text it starts from. Return: the text, or None when an action failed (a model run that did not finish or was cancelled): the rule then does nothing, so a later step never works on text an earlier one never produced and its output is not routed.
fn apply_steps(rule: &CompiledRule, text: &str) -> Option<String> {
    let mut now = text.to_string();
    for action in &rule.rule.actions {
        match action.step(&now) {
            Step::Edited(next) => now = next,
            Step::Unchanged => {}
            Step::Failed => return None,
        }
    }
    Some(now)
}

/// Runs the rules for `trigger` over `text`; when the text is over `MAX_EVENT_BYTES` (at the start, or after an earlier rule grew it) the chain stops there, the text so far is kept and `skipped` is set. A rule that changes nothing still counts as matched (it may route the output).
pub fn run(rules: &[CompiledRule], trigger: Trigger, text: &str, app: &dyn Fn() -> String) -> Outcome {
    let mut out = Outcome { text: text.to_string(), output: Output::Insert, matched: Vec::new(), skipped: false };
    for rule in rules {
        if out.text.len() > MAX_EVENT_BYTES && rule.rule.enabled && rule.rule.trigger == trigger {
            out.skipped = true;
            break;
        }
        if !rule.matches(trigger, &out.text, app) {
            continue;
        }
        let before = out.text.clone();
        let Some(after) = apply_steps(rule, &before) else {
            out.text = before;
            break;
        };
        out.text = after;
        out.output = rule.rule.output;
        out.matched.push(rule.rule.name.clone());
        if rule.rule.stop {
            break;
        }
    }
    out
}
