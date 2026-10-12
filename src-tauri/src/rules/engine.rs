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

fn within_limits(rule: &Rule) -> Result<(), String> {
    let too_long = |s: &str| s.chars().count() > MAX_TEXT;
    let text_too_long = rule.conditions.iter().any(|c| match c {
        Condition::Contains { value } | Condition::StartsWith { value } | Condition::EndsWith { value } => too_long(value),
        Condition::Matches { pattern } => too_long(pattern),
        _ => false,
    });
    let name = &rule.name;
    if name.chars().count() > MAX_NAME
        || rule.actions.len() > MAX_ACTIONS
        || rule.conditions.len() > MAX_CONDITIONS
        || rule.apps.len() > MAX_APPS
        || rule.apps.iter().any(|a| too_long(a))
        || text_too_long
    {
        return Err(format!(
            "\"{name}\" is too large: at most {MAX_ACTIONS} actions, {MAX_CONDITIONS} conditions and {MAX_TEXT} characters in a condition."
        ));
    }
    Ok(())
}

pub fn compile(rule: Rule) -> Result<CompiledRule, String> {
    within_limits(&rule)?;
    if rule.name.trim().is_empty() {
        return Err("Every rule needs a name.".into());
    }
    if rule.actions.is_empty() {
        return Err(format!("\"{}\" has no actions.", rule.name));
    }
    let mut conditions =
        rule.conditions.iter().map(condition::compile).collect::<Result<Vec<_>, _>>().map_err(|e| format!("\"{}\": {e}", rule.name))?;
    conditions.sort_by_key(condition::cost);
    let apps = rule.apps.iter().map(|a| normalize_app(a)).collect();
    let needs_lower = conditions.iter().any(condition::needs_lower);
    Ok(CompiledRule { rule, conditions, needs_lower, apps })
}

fn normalize_app(name: &str) -> String {
    let name = name.trim().to_lowercase();
    name.strip_suffix(".exe").map(String::from).unwrap_or(name)
}

impl CompiledRule {
    fn matches(&self, trigger: Trigger, text: &str, app: &dyn Fn() -> String) -> bool {
        if !self.rule.enabled || self.rule.trigger != trigger {
            return false;
        }
        // The program is looked up only when a rule is limited to programs, and only after the cheaper checks passed.
        self.conditions_hold(text) && (self.apps.is_empty() || self.apps.contains(&app()))
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::Action;
    use crate::rules::model::{Condition, Kind};

    fn rule(name: &str, conditions: Vec<Condition>, actions: Vec<Action>) -> CompiledRule {
        compile(Rule {
            name: name.into(),
            enabled: true,
            trigger: Trigger::Dictation,
            apps: vec![],
            when: Match::All,
            conditions,
            actions,
            output: Output::Insert,
            stop: false,
        })
        .unwrap()
    }

    fn none() -> String {
        String::new()
    }

    #[test]
    fn an_oversized_rule_is_refused() {
        let with = |conditions: Vec<Condition>, actions: Vec<Action>| Rule {
            name: "big".into(),
            enabled: true,
            trigger: Trigger::Dictation,
            apps: vec![],
            when: Match::All,
            conditions,
            actions,
            output: Output::Insert,
            stop: false,
        };
        let text = |value: String| vec![Condition::Contains { value }];
        assert!(compile(with(text("x".repeat(501)), vec![Action::UpperCase])).is_err());
        assert!(compile(with(text("ok".into()), vec![Action::UpperCase])).is_ok());
        assert!(compile(with(vec![], vec![Action::UpperCase; 21])).is_err());
    }

    #[test]
    fn matching_rules_chain_in_order() {
        let rules = [
            rule("links", vec![Condition::LooksLike { kind: Kind::Url }], vec![Action::CleanLinks]),
            rule("shout", vec![Condition::Contains { value: "a.com".into() }], vec![Action::UpperCase]),
        ];
        let out = run(&rules, Trigger::Dictation, "https://a.com/p?utm_source=x", &none);
        assert_eq!(out.text, "HTTPS://A.COM/P");
        assert_eq!(out.matched, ["links", "shout"]);
    }

    #[test]
    fn stop_and_trigger_and_enabled_are_respected() {
        let mut first = rule("first", vec![], vec![Action::LowerCase]);
        first.rule.stop = true;
        let rules = [first, rule("second", vec![], vec![Action::UpperCase])];
        assert_eq!(run(&rules, Trigger::Dictation, "AbC", &none).text, "abc");
        assert!(run(&rules, Trigger::Selection, "AbC", &none).matched.is_empty());
        let mut off = rule("off", vec![], vec![Action::UpperCase]);
        off.rule.enabled = false;
        assert!(run(&[off], Trigger::Dictation, "x", &none).matched.is_empty());
    }

    #[test]
    fn app_scope_looks_the_program_up_lazily() {
        let mut scoped = rule("slack only", vec![], vec![Action::UpperCase]);
        scoped.apps = vec!["slack".into()];
        let rules = [scoped];
        assert_eq!(run(&rules, Trigger::Dictation, "hi", &|| "slack".into()).text, "HI");
        assert_eq!(run(&rules, Trigger::Dictation, "hi", &|| "notepad".into()).text, "hi");
        let unscoped = [rule("any", vec![], vec![Action::UpperCase])];
        run(&unscoped, Trigger::Dictation, "hi", &|| panic!("must not look up the program"));
    }

    #[test]
    fn any_and_none_combine_conditions() {
        let mut any = rule(
            "any",
            vec![Condition::Contains { value: "x".into() }, Condition::Contains { value: "y".into() }],
            vec![Action::UpperCase],
        );
        any.rule.when = Match::Any;
        assert!(!run(&[any], Trigger::Dictation, "z", &none).matched.contains(&"any".to_string()));
        let mut not = rule("not", vec![Condition::Contains { value: "x".into() }], vec![Action::UpperCase]);
        not.rule.when = Match::None;
        assert_eq!(run(&[not], Trigger::Dictation, "abc", &none).text, "ABC");
    }

    #[test]
    fn invalid_rules_are_refused_with_a_reason() {
        let bad = Rule {
            name: "r".into(),
            enabled: true,
            trigger: Trigger::Dictation,
            apps: vec![],
            when: Match::All,
            conditions: vec![Condition::Matches { pattern: "(".into() }],
            actions: vec![Action::UpperCase],
            output: Output::Insert,
            stop: false,
        };
        assert!(compile(bad).err().unwrap().contains("not a valid pattern"));
    }
}
