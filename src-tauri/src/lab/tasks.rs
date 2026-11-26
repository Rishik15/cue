//! File: lab/tasks.rs
//! Purpose: `cue --lab dump-tasks`: every model task as production sends it, so the lab scripts never read prompts from Rust source.
//! Contents: dump — the guard sentence and each model action's system text and worked examples as JSON.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::actions::{task, Action, GUARD};

/// The dump: the guard sentence and the tasks by action id.
#[derive(Serialize)]
struct Dump {
    guard: &'static str,
    tasks: BTreeMap<&'static str, Prompt>,
}

/// One task's prompt: the system text and the worked examples flat as in, out, in, out, so the lab's chat-turn loops read them pairwise.
#[derive(Serialize)]
struct Prompt {
    system: &'static str,
    shots: Vec<&'static str>,
}

/// Builds the dump for every action that has a model task. Returns `{"guard": ..., "tasks": {"<id>": {"system": ..., "shots": [...]}}}`, or an error when serializing fails.
pub fn dump() -> Result<String, String> {
    let tasks = Action::all()
        .filter_map(|a| {
            task(a).map(|t| (a.id(), Prompt { system: t.system, shots: t.shots.iter().flat_map(|(said, want)| [*said, *want]).collect() }))
        })
        .collect();
    serde_json::to_string(&Dump { guard: GUARD, tasks }).map_err(|e| format!("cannot write the tasks: {e}"))
}
