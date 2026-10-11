//! Purpose: Rules that route text events through exact actions: load and save `rules.json`, run the matching rules for an event, and
//! offer a dry run for the Rules page. Costs nothing until a rule exists: the file is read once at start and an empty list is skipped.
//! Contents: init — load the saved rules; run — apply the rules for a trigger to a text and note which matched; get_rules / save_rules / test_rules / rule_log — commands.
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

fn compile_all(rules: Vec<Rule>) -> Result<Vec<CompiledRule>, String> {
    rules.into_iter().map(compile).collect()
}

fn read(app: &AppHandle) -> Result<Vec<Rule>, String> {
    let Ok(text) = std::fs::read_to_string(path(app)?) else { return Ok(Vec::new()) }; // no file yet: no rules
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
pub fn save_rules(app: AppHandle, rules: Vec<Rule>) -> Result<(), String> {
    let compiled = compile_all(rules.clone())?;
    let _one_at_a_time = SAVING.lock().unwrap();
    let path = path(&app)?;
    let json = serde_json::to_string_pretty(&File { version: VERSION, rules }).map_err(|e| e.to_string())?;
    let temp = path.with_extension("json.tmp");
    std::fs::create_dir_all(path.parent().unwrap_or(&path)).map_err(|e| format!("Could not save rules: {e}"))?;
    std::fs::write(&temp, json).and_then(|()| std::fs::rename(&temp, &path)).map_err(|e| format!("Could not save rules: {e}"))?;
    *RULES.write().unwrap() = Arc::new(compiled);
    Ok(())
}

#[derive(Serialize)]
pub struct TestResult {
    text: String,
    output: Output,
    matched: Vec<String>,
}

/// Dry run for the Rules page: the unsaved rules, a sample text and a pretend program. Touches nothing.
#[tauri::command]
pub fn test_rules(rules: Vec<Rule>, trigger: Trigger, text: String, app: String) -> Result<TestResult, String> {
    let compiled = compile_all(rules)?;
    let out = engine::run(&compiled, trigger, &text, &|| app.trim().to_lowercase());
    Ok(TestResult { text: out.text, output: out.output, matched: out.matched })
}
