//! File: rules/mod.rs
//! Purpose: Rules that route text events through exact actions: hold the saved rules and run the matching ones for an event. Costs nothing until a rule exists: the file is read once at start and an empty list is skipped.
//! Contents: init — load the saved rules; publish — make a compiled list the current one; run — apply the rules for a trigger to a text and note which matched; Chosen — one saved action offered, fixed at the offer (run, record, output); matching — the choices a selection press offers; choose — a row id back to its choice while it is still current; model_task_for — the first text-model task a trigger would run.
//! `model` is the rule shape, `storage` the rules.json file, `commands` what the Rules page calls, `condition` the match tests, `engine` the pure evaluation, `log` the run history.

pub mod commands;
mod condition;
mod engine;
mod log;
pub mod model;
mod storage;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, OnceLock, RwLock};

use tauri::AppHandle;

use crate::{platform, status};
pub use engine::Outcome;
use engine::{compile_all, CompiledRule};
use model::{Output, Trigger};

static RULES: LazyLock<RwLock<Arc<Vec<CompiledRule>>>> = LazyLock::new(|| RwLock::new(Arc::new(Vec::new())));

/// Which published list of saved actions `RULES` holds: it moves on at every publish, under the same lock, so a choice made from one list is told apart from the next.
static REVISION: AtomicU64 = AtomicU64::new(0);

/// Makes `rules` the saved actions and moves the revision on. Input: the compiled rules.
fn publish(rules: Vec<CompiledRule>) {
    let mut held = RULES.write().unwrap_or_else(|e| e.into_inner());
    *held = Arc::new(rules);
    REVISION.fetch_add(1, Ordering::AcqRel);
}

/// A broken file never stops Cue: it is reported and rules stay off until it is fixed or replaced by saving from the Rules page.
pub fn init(app: &AppHandle) {
    match storage::read(app).and_then(compile_all) {
        Ok(rules) => publish(rules),
        Err(e) => status::report(app, &e),
    }
}

/// The text after every matching rule for `trigger` (one program lookup is shared by scoped rules and the run history). The program in front is looked up only if a matching rule is limited to programs.
pub fn run(trigger: Trigger, text: &str) -> Outcome {
    let rules = RULES.read().unwrap().clone();
    let app = OnceLock::new();
    let lookup = || app.get_or_init(platform::foreground_app).clone();
    let out = engine::run(&rules, trigger, text, &lookup);
    if !out.matched.is_empty() {
        log::record(trigger, &out.matched, lookup());
    }
    out
}

/// The action labels of a rule joined for display. Input: the compiled rule. Returns the text.
fn step_labels(rule: &engine::CompiledRule) -> String {
    rule.rule.actions.iter().map(|a| a.label()).collect::<Vec<_>>().join(" · ")
}

/// One saved action the user was offered, fixed at the moment of the offer: the list it came from (kept alive by the choice), its place in it and that list's revision. Running it, noting it and routing its result all read the same rule of the same list, so an edit, a reorder or a removal made while a palette is open can never make one step act on another rule; a later step with the old id finds the revision moved on and is refused (`choose`).
pub struct Chosen {
    rules: Arc<Vec<CompiledRule>>,
    at: usize,
    revision: u64,
}

impl Chosen {
    /// The rule this choice is.
    fn rule(&self) -> &CompiledRule {
        &self.rules[self.at]
    }

    /// The row id tail that names this choice: `<revision>:<position>:<name>`.
    pub fn tail(&self) -> String {
        format!("{}:{}:{}", self.revision, self.at, self.rule().rule.name)
    }

    /// The action's name.
    pub fn name(&self) -> &str {
        &self.rule().rule.name
    }

    /// The step labels, for the list's second line.
    pub fn steps(&self) -> String {
        step_labels(self.rule())
    }

    /// Whether the result is shown as a word diff: only when every step is one that fixes a few words (`Action::diffs`), so a saved rewrite or translation shows the plain result. Return: the answer.
    pub fn diffs(&self) -> bool {
        crate::actions::chain_diffs(&self.rule().rule.actions)
    }

    /// Whether a step uses the text model, so a result takes seconds and is not computed up front.
    pub fn model(&self) -> bool {
        self.rule().rule.actions.iter().any(|a| a.needs_model())
    }

    /// Where the result goes (insert, copy or preview).
    pub fn output(&self) -> Output {
        self.rule().rule.output
    }

    /// Runs the rule on `text` without noting it in the run log: the palette previews many rules, and only the one the user applies counts (`record`). `app` is the program the text came from: by the time a slow preview runs, the foreground window is Cue's own, so a rule scoped to programs would never match.
    pub fn run(&self, text: &str, app: &str) -> Outcome {
        let rule = self.rule();
        engine::run(std::slice::from_ref(rule), rule.rule.trigger, text, &|| app.to_string())
    }

    /// Notes in the run log that the rule was applied to text from `app`.
    pub fn record(&self, app: &str) {
        let rule = self.rule();
        log::record(rule.rule.trigger, std::slice::from_ref(&rule.rule.name), app.to_string());
    }
}

/// The enabled rules for `trigger` whose conditions match `text` as it stands, in saved order, each as one fixed choice. The selection palette lists these first.
pub fn matching(trigger: Trigger, text: &str) -> Vec<Chosen> {
    let (rules, revision) = {
        let held = RULES.read().unwrap_or_else(|e| e.into_inner());
        (held.clone(), REVISION.load(Ordering::Acquire))
    };
    let app = OnceLock::new();
    let lookup = || app.get_or_init(platform::foreground_app).clone();
    let found: Vec<usize> =
        rules.iter().enumerate().filter(|(_, r)| r.rule.enabled && r.matches(trigger, text, &lookup)).map(|(at, _)| at).collect();
    found.into_iter().map(|at| Chosen { rules: rules.clone(), at, revision }).collect()
}

/// The choice a palette row id names (its tail after `rule:`), only while the list it was made from is still the saved one and the rule there still has that name: saving rules while a palette is open moves the list on, and the old choice is then refused instead of landing on another rule. Input: the tail. Return: the choice, or None when it is stale or not a choice.
pub fn choose(tail: &str) -> Option<Chosen> {
    let mut parts = tail.splitn(3, ':');
    let revision: u64 = parts.next()?.parse().ok()?;
    let at: usize = parts.next()?.parse().ok()?;
    let name = parts.next()?;
    let held = RULES.read().unwrap_or_else(|e| e.into_inner());
    if REVISION.load(Ordering::Acquire) != revision || held.get(at)?.rule.name != name {
        return None;
    }
    Some(Chosen { rules: held.clone(), at, revision })
}

/// The task of the first text-model action in an enabled rule for `trigger`, so the model can warm up before the text exists (while the user speaks or the copy runs).
pub fn model_task_for(trigger: Trigger) -> Option<&'static crate::actions::Task> {
    let rules = RULES.read().unwrap();
    let mut enabled = rules.iter().filter(|r| r.rule.enabled && r.rule.trigger == trigger);
    enabled.find_map(|r| r.rule.actions.iter().find_map(|a| crate::actions::task(*a)))
}

#[cfg(test)]
mod tests;
