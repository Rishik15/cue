//! Purpose: Rules that route text events through exact actions: load and save `rules.json`, run the matching rules for an event, and
//! offer a dry run for the Rules page. Costs nothing until a rule exists: the file is read once at start and an empty list is skipped.
//! Contents: init — load the saved rules; run — apply the rules for a trigger to a text and note which matched; matching / run_one / record_run / output_of — the saved actions a selection press offers, running one, noting an applied one, and where its result goes; model_task_for — the first text-model task a trigger would run; get_rules / save_rules / test_rules / rule_log — commands.
//! `model` is the file format, `condition` the match tests, `engine` the pure evaluation.

mod condition;
mod engine;
mod log;
pub mod model;

use std::path::PathBuf;
use std::sync::{Arc, LazyLock, Mutex, OnceLock, RwLock};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::{platform, status};
pub use engine::Outcome;
use engine::{compile, CompiledRule};
use model::{Output, Rule, Trigger};

const VERSION: u32 = 1;

/// Saves come from a page that can fire several at once (switching rules on and off quickly); one at a time keeps the temp file and the
/// order of the in-memory swap right.
static SAVING: Mutex<()> = Mutex::new(());

static RULES: LazyLock<RwLock<Arc<Vec<CompiledRule>>>> = LazyLock::new(|| RwLock::new(Arc::new(Vec::new())));

#[derive(Serialize, Deserialize)]
struct File {
    version: u32,
    rules: Vec<Rule>,
}

fn path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app.path().app_data_dir().map_err(|e| e.to_string())?.join("rules.json"))
}

/// More rules than anyone writes by hand; the cap keeps a runaway file or page from filling memory.
const MAX_RULES: usize = 200;
/// The largest `rules.json` Cue will read.
const MAX_FILE_BYTES: u64 = 1 << 20;

fn compile_all(rules: Vec<Rule>) -> Result<Vec<CompiledRule>, String> {
    if rules.len() > MAX_RULES {
        return Err(format!("Too many rules (at most {MAX_RULES})."));
    }
    rules.into_iter().map(compile).collect()
}

fn read(app: &AppHandle) -> Result<Vec<Rule>, String> {
    let path = path(app)?;
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > MAX_FILE_BYTES) {
        return Err("rules.json is too large to read.".into());
    }
    let Ok(text) = std::fs::read_to_string(path) else { return Ok(Vec::new()) }; // no file yet: no rules
    let file: File = serde_json::from_str(&text).map_err(|e| format!("rules.json is not valid: {e}"))?;
    if file.version > VERSION {
        return Err("rules.json was saved by a newer Cue. Update Cue to use these rules.".into());
    }
    Ok(file.rules)
}

/// A broken file never stops Cue: it is reported and rules stay off until it is fixed or replaced by saving from the Rules page.
pub fn init(app: &AppHandle) {
    match read(app).and_then(compile_all) {
        Ok(rules) => *RULES.write().unwrap() = Arc::new(rules),
        Err(e) => status::report(app, &e),
    }
}

/// The text after every matching rule for `trigger`. The program in front is looked up only if a matching rule is limited to programs.
pub fn run(trigger: Trigger, text: &str) -> Outcome {
    let rules = RULES.read().unwrap().clone();
    let app = OnceLock::new(); // one lookup shared by scoped rules and the run history
    let lookup = || app.get_or_init(platform::foreground_app).clone();
    let out = engine::run(&rules, trigger, text, &lookup);
    if !out.matched.is_empty() {
        log::record(trigger, &out.matched, lookup());
    }
    out
}

fn step_labels(rule: &engine::CompiledRule) -> String {
    rule.rule.actions.iter().map(|a| a.label()).collect::<Vec<_>>().join(" · ")
}

/// One action the selection shortcut can offer for a text.
pub struct Candidate {
    /// Position in the saved list, which `run_one` takes.
    pub at: usize,
    pub name: String,
    /// The step labels, for the list's second line.
    pub steps: String,
    /// A step uses the text model, so a result takes seconds and is not computed up front.
    pub model: bool,
}

/// The enabled rules for `trigger` whose conditions match `text` as it stands, in saved order. The selection palette lists these first.
pub fn matching(trigger: Trigger, text: &str) -> Vec<Candidate> {
    let rules = RULES.read().unwrap().clone();
    let app = OnceLock::new();
    let lookup = || app.get_or_init(platform::foreground_app).clone();
    let found = rules.iter().enumerate().filter(|(_, r)| r.rule.enabled && r.matches(trigger, text, &lookup));
    found
        .map(|(at, r)| Candidate {
            at,
            name: r.rule.name.clone(),
            steps: step_labels(r),
            model: r.rule.actions.iter().any(|a| a.needs_model()),
        })
        .collect()
}

/// Where the result of the rule at `at` goes (insert, copy or preview).
pub fn output_of(at: usize) -> Option<Output> {
    RULES.read().unwrap().get(at).map(|r| r.rule.output)
}

/// Runs the one rule at `at` on `text` without noting it in the run log: the palette previews many rules, and only the one the user applies counts (`record_run`).
/// `app` is the program the text came from: by the time a slow preview runs, the foreground window is Cue's own, so a rule scoped to programs would never match.
pub fn run_one(at: usize, text: &str, app: &str) -> Option<Outcome> {
    let rules = RULES.read().unwrap().clone();
    let rule = rules.get(at)?;
    Some(engine::run(std::slice::from_ref(rule), rule.rule.trigger, text, &|| app.to_string()))
}

/// Notes in the run log that the rule at `at` was applied to text from `app`.
pub fn record_run(at: usize, app: &str) {
    if let Some(rule) = RULES.read().unwrap().get(at) {
        log::record(rule.rule.trigger, std::slice::from_ref(&rule.rule.name), app.to_string());
    }
}

/// The task of the first text-model action in an enabled rule for `trigger`, so the model can warm up before the text exists (while the user speaks or the copy runs).
pub fn model_task_for(trigger: Trigger) -> Option<&'static crate::actions::Task> {
    let rules = RULES.read().unwrap();
    let mut enabled = rules.iter().filter(|r| r.rule.enabled && r.rule.trigger == trigger);
    enabled.find_map(|r| r.rule.actions.iter().find_map(|a| crate::actions::task(*a)))
}

/// The most recent matching runs, newest first. No text is kept.
#[tauri::command]
pub fn rule_log() -> Vec<log::Entry> {
    log::recent()
}

#[tauri::command]
pub fn get_rules(app: AppHandle) -> Result<Vec<Rule>, String> {
    read(&app)
}

/// Validates every rule, then writes the file in one step (temp file, then rename) so a crash cannot leave half a file.
#[tauri::command]
pub async fn save_rules(app: AppHandle, rules: Vec<Rule>) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || save(&app, rules)).await.map_err(|e| e.to_string())?
}

fn save(app: &AppHandle, rules: Vec<Rule>) -> Result<(), String> {
    let compiled = compile_all(rules.clone())?;
    let _one_at_a_time = SAVING.lock().unwrap();
    let path = path(app)?;
    let json = serde_json::to_string_pretty(&File { version: VERSION, rules }).map_err(|e| e.to_string())?;
    let temp = path.with_extension("json.tmp");
    std::fs::create_dir_all(path.parent().unwrap_or(&path)).map_err(|e| format!("Could not save rules: {e}"))?;
    // Written, flushed to disk, then renamed over the old file: a crash or a full disk leaves the previous rules intact, and the temp file never lingers.
    write_synced(&temp, &json).and_then(|()| std::fs::rename(&temp, &path)).map_err(|e| {
        let _ = std::fs::remove_file(&temp);
        format!("Could not save rules: {e}")
    })?;
    *RULES.write().unwrap() = Arc::new(compiled);
    Ok(())
}

fn write_synced(path: &std::path::Path, text: &str) -> std::io::Result<()> {
    use std::io::Write;
    let mut file = std::fs::File::create(path)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()
}

#[derive(Serialize)]
pub struct TestResult {
    text: String,
    output: Output,
    matched: Vec<String>,
}

/// The dry run behind the editor's Test panel. Without `with_model` the text-model steps are left out, because the model would load on every
/// keystroke; a rule made only of them then has nothing left to run and drops out instead of failing validation with "has no actions"
/// (a rule that was empty to begin with still reports it).
fn dry_run(mut rules: Vec<Rule>, trigger: Trigger, text: &str, app: &str, with_model: bool) -> Result<TestResult, String> {
    if !with_model {
        rules.retain_mut(|r| {
            let had_steps = !r.actions.is_empty();
            r.actions.retain(|a| !a.needs_model());
            !had_steps || !r.actions.is_empty()
        });
    }
    let compiled = compile_all(rules)?;
    let out = engine::run(&compiled, trigger, text, &|| app.trim().to_lowercase());
    Ok(TestResult { text: out.text, output: out.output, matched: out.matched })
}

/// Test panel command: the unsaved rules, a sample text and a pretend program. Touches nothing. `with_model` runs the text model's steps too
/// (the "Run With Text Model" button), which can take seconds, so it runs off the main thread.
#[tauri::command]
pub async fn test_rules(rules: Vec<Rule>, trigger: Trigger, text: String, app: String, with_model: bool) -> Result<TestResult, String> {
    tauri::async_runtime::spawn_blocking(move || dry_run(rules, trigger, &text, &app, with_model))
        .await
        .map_err(|e| format!("The test did not finish: {e}"))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::Action;

    fn rule(actions: Vec<Action>) -> Rule {
        serde_json::from_value(serde_json::json!({ "name": "r", "trigger": "selection", "actions": actions })).unwrap()
    }

    #[test]
    fn a_rule_of_only_model_steps_is_skipped_in_the_test() {
        let out = dry_run(vec![rule(vec![Action::BulletList])], Trigger::Selection, "a, b", "", false).unwrap();
        assert!(out.matched.is_empty());
    }

    #[test]
    fn matching_offers_only_enabled_rules_whose_conditions_fit() {
        let with = |name: &str, extra: serde_json::Value| -> Rule {
            let mut v = serde_json::json!({ "name": name, "trigger": "selection", "actions": ["upper_case"] });
            v.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
            serde_json::from_value(v).unwrap()
        };
        let list = vec![
            with("any", serde_json::json!({})),
            with("needs http", serde_json::json!({ "conditions": [{ "type": "contains", "value": "http" }] })),
            with("off", serde_json::json!({ "enabled": false })),
        ];
        *RULES.write().unwrap() = Arc::new(compile_all(list).unwrap());
        let names = |text: &str| matching(Trigger::Selection, text).into_iter().map(|c| c.name).collect::<Vec<_>>();
        assert_eq!(names("plain"), ["any"]);
        assert_eq!(names("see http://a.b"), ["any", "needs http"]);
        assert_eq!(matching(Trigger::Selection, "plain")[0].at, 0);
        *RULES.write().unwrap() = Arc::new(Vec::new());
    }

    #[test]
    fn a_rule_with_no_steps_still_reports_it() {
        let Err(err) = dry_run(vec![rule(vec![])], Trigger::Selection, "a", "", false) else { panic!("an empty rule must fail") };
        assert!(err.contains("has no actions"));
    }
}
