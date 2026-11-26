//! File: lab/mod.rs
//! Purpose: `cue --lab <command> [arg]`: the app surface the lab scripts (`scripts/lab/`) read instead of Rust source, so a bench sends the prompts and guard sentence production sends and judges answers with production's checks.
//! No UI and no idle cost: `main` calls it before Tauri, the single-instance plugin or the keyboard hook start, and the process exits after one command.
//! Contents: run — dispatch one command and print its result to stdout (UTF-8) or `cue: <message>` to stderr; dispatch — the command table; read — the text of the file a command names.
//! Commands: `dump-tasks` (tasks), `judge <file>` (judge), `clean <file>` (clean), `installed <models folder>` (installed).

mod clean;
mod installed;
mod judge;
mod tasks;
#[cfg(test)]
mod tests;

use std::io::Write;
use std::path::Path;

/// Runs one lab command. Input: the words after `--lab`. Returns the exit code: 0 on success, 2 for an unknown command, a missing or unreadable file, or invalid JSON.
pub fn run(args: &[String]) -> i32 {
    let written =
        dispatch(args).and_then(|out| std::io::stdout().write_all(out.as_bytes()).map_err(|e| format!("cannot write the result: {e}")));
    match written {
        Ok(()) => 0,
        Err(message) => {
            eprintln!("cue: {message}");
            2
        }
    }
}

/// Picks and runs the command named by the first word. Input: the words after `--lab`. Returns the text to print, or a user-readable error.
fn dispatch(args: &[String]) -> Result<String, String> {
    let arg = || args.get(1).ok_or_else(|| "this lab command needs a path after it".to_string());
    match args.first().map(String::as_str) {
        Some("dump-tasks") => tasks::dump(),
        Some("judge") => judge::judge_rows(&read(arg()?)?),
        Some("clean") => clean::clean_strings(&read(arg()?)?),
        Some("installed") => installed::list(Path::new(arg()?)),
        Some(other) => {
            Err(format!("unknown lab command {other:?}; commands: dump-tasks, judge <file>, clean <file>, installed <models folder>"))
        }
        None => Err("--lab needs a command: dump-tasks, judge <file>, clean <file>, installed <models folder>".into()),
    }
}

/// Reads the file a command names. Input: its path. Returns its text, or an error naming the path.
fn read(path: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))
}
