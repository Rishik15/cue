//! Purpose: The tasks behind the actions that use the text model. Everything else about an action (id, label, hint) lives in `Action`.
//! Contents: prompt — the task for a model action, None for exact actions; needs_model — whether an action uses the model; set_runner / run — plug in and call whatever answers model tasks.

use std::sync::OnceLock;

use super::Action;

pub fn prompt(action: Action) -> Option<&'static str> {
    Some(match action {
        Action::Correct => {
            "Fix spelling, grammar and punctuation. Keep the meaning, wording, tone, language and layout. Change nothing else."
        }
        Action::MakeShorter => "Make the text shorter and clearer. Keep every fact, the meaning and the language.",
        Action::MakeFormal => "Rewrite the text in a formal, professional tone. Keep every fact, the meaning and the language.",
        Action::MakeFriendly => "Rewrite the text in a warm, friendly, relaxed tone. Keep every fact, the meaning and the language.",
        _ => return None,
    })
}

pub fn needs_model(action: Action) -> bool {
    prompt(action).is_some()
}

type Runner = fn(&str, &str) -> Option<String>;

/// Who answers model tasks. Registered at start by `text::init`, so exact actions and their tests never link the text engine (and through
/// it Tauri's runtime, whose comctl32 v6 imports cannot load in a test executable that has no application manifest).
static RUNNER: OnceLock<Runner> = OnceLock::new();

pub fn set_runner(runner: Runner) {
    let _ = RUNNER.set(runner);
}

pub fn run(task: &str, text: &str) -> Option<String> {
    RUNNER.get().and_then(|runner| runner(task, text))
}
