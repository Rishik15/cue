//! File: rules/commands.rs
//! Purpose: What the Rules page calls: read the saved rules, save them, run the dry run behind the Test panel and read the run history.
//! Contents: get_rules / save_rules / test_rules / rule_log — the commands; dry_run — the Test panel's run on unsaved rules; TestResult — its answer.

use serde::Serialize;
use tauri::AppHandle;

use super::engine::{self, compile_all};
use super::model::{Output, Rule, Trigger};
use super::{log, publish, storage};

/// The most recent matching runs, newest first. No text is kept.
#[tauri::command]
pub fn rule_log() -> Vec<log::Entry> {
    log::recent()
}

/// The rules in the saved file, for the Rules page; the file is read on a worker thread, never the main one. Input: app handle. Return: the rules, or why the file cannot be used.
#[tauri::command]
pub async fn get_rules(app: AppHandle) -> Result<Vec<Rule>, String> {
    tauri::async_runtime::spawn_blocking(move || storage::read(&app)).await.map_err(|e| e.to_string())?
}

/// Validates every rule, then writes the file and swaps the compiled rules in (`storage::save_to`).
#[tauri::command]
pub async fn save_rules(app: AppHandle, rules: Vec<Rule>) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || storage::save_to(&storage::path(&app)?, rules, publish))
        .await
        .map_err(|e| e.to_string())?
}

/// The Test panel's answer: the text after the run, where it would go and the names of the rules that matched.
#[derive(Serialize)]
pub struct TestResult {
    pub(super) text: String,
    pub(super) output: Output,
    pub(super) matched: Vec<String>,
}

/// The dry run behind the editor's Test panel. Without `with_model` the text-model steps are left out, because the model would load on every
/// keystroke; a rule made only of them then has nothing left to run and drops out instead of failing validation with "has no actions"
/// (a rule that was empty to begin with still reports it).
pub fn dry_run(mut rules: Vec<Rule>, trigger: Trigger, text: &str, app: &str, with_model: bool) -> Result<TestResult, String> {
    if !with_model {
        rules.retain_mut(|r| {
            let had_steps = !r.actions.is_empty();
            r.actions.retain(|a| !a.needs_model());
            !had_steps || !r.actions.is_empty()
        });
    }
    let compiled = compile_all(rules)?;
    let app = engine::normalize_app(app);
    let out = engine::run(&compiled, trigger, text, &|| app.clone());
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
