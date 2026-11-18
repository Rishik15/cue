//! File: rules/storage.rs
//! Purpose: The rules.json file: its format and version, reading it, setting a damaged one aside and saving without ever losing the old one.
//! Contents: File / VERSION — the stored shape and its version; path — where it lives; read — load and decode it; decode — the version match; set_aside — keep a damaged file; keeps_file — why a save must not replace the file; save_to — validate and write it atomically.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use super::engine::{compile_all, CompiledRule};
use super::model::Rule;

/// The version of the format written today. Older versions need their own arm in `decode` before this number moves on.
pub const VERSION: u32 = 1;

/// The largest `rules.json` Cue will read.
const MAX_FILE_BYTES: u64 = 1 << 20;

/// Saves come from a page that can fire several at once (switching rules on and off quickly); one at a time keeps the temp file and the order of the in-memory swap right.
static SAVING: Mutex<()> = Mutex::new(());

#[derive(Serialize, Deserialize)]
pub struct File {
    pub version: u32,
    pub rules: Vec<Rule>,
}

/// Only the version of a file, read first: a newer or older file may have a shape this build cannot parse, and must be told apart from a damaged one.
#[derive(Deserialize)]
struct Versioned {
    version: u32,
}

/// Where `rules.json` lives. Input: app handle. Returns the path, or Err when the data folder is unknown.
pub fn path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app.path().app_data_dir().map_err(|e| e.to_string())?.join("rules.json"))
}

/// Why a file's text is not rules this build can use. `Damaged` carries the parse error so a damaged file can be set aside; the others leave the file where it is.
pub enum Unusable {
    Damaged(serde_json::Error),
    Newer,
    Older(u32),
}

/// Reads the rules out of a file's text by its version. Input: the text. Returns the rules, or why it cannot be used: version 1 is the only one that exists, a higher one is from a newer Cue and a lower one was never written by any Cue.
pub fn decode(text: &str) -> Result<Vec<Rule>, Unusable> {
    let Versioned { version } = serde_json::from_str(text).map_err(Unusable::Damaged)?;
    match version {
        VERSION => Ok(serde_json::from_str::<File>(text).map_err(Unusable::Damaged)?.rules),
        v if v > VERSION => Err(Unusable::Newer),
        v => Err(Unusable::Older(v)),
    }
}

/// The message for a file that cannot be used, without any rule text. Input: why. Returns the message.
fn unusable_message(why: &Unusable) -> String {
    match why {
        Unusable::Newer => "rules.json was saved by a newer Cue. Update Cue to use these rules.".into(),
        Unusable::Older(v) => format!("rules.json has version {v}, which this Cue does not know."),
        Unusable::Damaged(_) => "rules.json is not valid.".into(),
    }
}

/// Reads and decodes `rules.json`. Input: app handle. Returns the rules (none when there is no file), or Err for a too-large, unreadable, damaged, older or newer file; damage is described by kind and position, never by the offending value.
pub fn read(app: &AppHandle) -> Result<Vec<Rule>, String> {
    let path = path(app)?;
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > MAX_FILE_BYTES) {
        return Err("rules.json is too large to read.".into());
    }
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("rules.json could not be read: {e}")),
    };
    decode(&text).map_err(|why| match why {
        Unusable::Damaged(cause) => set_aside(&path, &cause),
        other => unusable_message(&other),
    })
}

/// The first name for a set-aside copy that is free: `rules.damaged.json`, then `rules.damaged-2.json` and so on, so an earlier recovery copy is never replaced. Input: the path of `rules.json`. Return: the free path.
fn free_copy_name(path: &Path) -> PathBuf {
    let first = path.with_file_name("rules.damaged.json");
    std::iter::once(first)
        .chain((2..).map(|n| path.with_file_name(format!("rules.damaged-{n}.json"))))
        .find(|p| !p.exists())
        .expect("endless range")
}

/// Moves a damaged `rules.json` to `rules.damaged.json` (or the next free name, `free_copy_name`): left in place, the next save from the Rules page would overwrite it and destroy the rules in it. Inputs: the file path and the parse error. Returns the message to show; it names the kind and position of the fault only, because serde's own text quotes the offending value and the message reaches `cue.log`.
pub fn set_aside(path: &Path, cause: &serde_json::Error) -> String {
    let copy = free_copy_name(path);
    let note = if std::fs::rename(path, &copy).is_ok() {
        format!(" It was kept as {}.", copy.file_name().and_then(|n| n.to_str()).unwrap_or("a copy"))
    } else {
        String::new()
    };
    format!("rules.json is not valid ({:?} error at line {}, column {}).{note}", cause.classify(), cause.line(), cause.column())
}

/// Why `rules.json` must not be replaced: it is there but cannot be read as rules (too large, unreadable, damaged, older or newer), so a save would destroy what the user has in it. A missing file is free to write. Input: the path. Return: the reason, or None.
fn keeps_file(path: &Path) -> Option<String> {
    if std::fs::metadata(path).is_ok_and(|m| m.len() > MAX_FILE_BYTES) {
        return Some("rules.json is too large to read, so it was not replaced.".into());
    }
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        Err(e) => return Some(format!("rules.json could not be read ({e}), so it was not replaced.")),
    };
    match decode(&text) {
        Ok(_) => None,
        Err(Unusable::Newer) => {
            Some("rules.json was saved by a newer Cue, so it was not replaced. Update Cue to change these rules.".into())
        }
        Err(Unusable::Older(v)) => Some(format!("rules.json has version {v}, which this Cue does not know, so it was not replaced.")),
        Err(Unusable::Damaged(_)) => Some("rules.json is not valid, so it was not replaced.".into()),
    }
}

/// Validates the rules and writes `path` in one step (temp file, flushed, then renamed over the old file: a crash or a full disk leaves the previous rules intact and the temp file never lingers), one save at a time. The old file is kept when it cannot be read as rules (`keeps_file`), and the new one is refused when it would be larger than Cue reads back (`MAX_FILE_BYTES`). `publish` gets the compiled rules after the file is on disk, inside the same lock, so saves swap in the order they were written. Input: the path, the rules and `publish`. Return: Ok, or Err with the reason nothing was written.
pub fn save_to(path: &Path, rules: Vec<Rule>, publish: impl FnOnce(Vec<CompiledRule>)) -> Result<(), String> {
    let compiled = compile_all(rules.clone())?;
    let json = serde_json::to_string_pretty(&File { version: VERSION, rules }).map_err(|e| e.to_string())?;
    if json.len() as u64 > MAX_FILE_BYTES {
        return Err("These rules are too large to save. Remove some rules or shorten their conditions.".into());
    }
    let _one_at_a_time = SAVING.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(why) = keeps_file(path) {
        return Err(why);
    }
    let temp = path.with_extension("json.tmp");
    std::fs::create_dir_all(path.parent().unwrap_or(path)).map_err(|e| format!("Could not save rules: {e}"))?;
    write_synced(&temp, &json).and_then(|()| std::fs::rename(&temp, path)).map_err(|e| {
        let _ = std::fs::remove_file(&temp);
        format!("Could not save rules: {e}")
    })?;
    publish(compiled);
    Ok(())
}

/// Writes `text` to `path` and flushes it to disk. Inputs: the path and the text. Return: Ok, or the I/O error.
fn write_synced(path: &Path, text: &str) -> std::io::Result<()> {
    use std::io::Write;
    let mut file = std::fs::File::create(path)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()
}
