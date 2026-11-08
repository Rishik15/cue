//! File: log.rs
//! Purpose: Cue's activity log: one line per notable event (a grab and what it read, every problem shown to the user) in `cue.log` in the data folder, next to `crash.log`, so what Cue did can be read afterwards.
//! Contents: init — where the log lives; note — append one line; append — the shared writer (also used for `crash.log`): timestamped, cut so the whole entry fits the entry cap, written in one call, and moved aside past a size cap; frame / room_for — pure entry and rotation arithmetic.
//! Callers pass events and error text only, never what the user typed, said or named.
//! Nothing here runs while idle: a line is written only when something happens.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use tauri::{AppHandle, Manager};

static LOG: OnceLock<PathBuf> = OnceLock::new();
/// Held from the size check to the end of the write, so two threads cannot rotate the same file over each other's backup.
static WRITING: Mutex<()> = Mutex::new(());
/// A log past this size is moved aside (one old copy is kept), so a loop of events cannot fill the disk.
const MAX_LOG: u64 = 256 * 1024;
/// One whole entry, timestamp and line end included, never exceeds this many bytes, so the file stays near the cap whatever a message holds (a crash backtrace fits).
const MAX_ENTRY: usize = 8 * 1024;

/// Remembers where `cue.log` lives, creating the data folder. Input: the app handle. Returns nothing; without a data folder `note` does nothing.
pub fn init(app: &AppHandle) {
    if let Ok(dir) = app.path().app_data_dir() {
        let _ = std::fs::create_dir_all(&dir);
        let _ = LOG.set(dir.join("cue.log"));
    }
}

/// Appends one line to `cue.log`. Input: the line. Returns nothing.
pub fn note(line: &str) {
    if let Some(path) = LOG.get() {
        append(path, line);
    }
}

/// Builds one entry: `[seconds] text` and a line end, cut at a character boundary so the whole entry is at most `MAX_ENTRY` bytes. Inputs: the Unix time in seconds and the text. Returns the entry.
fn frame(at: u64, text: &str) -> String {
    let head = format!("[{at}] ");
    let mut end = text.len().min(MAX_ENTRY.saturating_sub(head.len() + 1));
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{head}{}\n", &text[..end])
}

/// Whether an entry fits in a log of the given size. Inputs: the log's current size and the entry's size in bytes. Returns the verdict.
fn room_for(size: u64, entry: u64) -> bool {
    size + entry <= MAX_LOG
}

/// Moves a full log aside. Input: the log path. Returns whether the log may be written: it is not full, or it was moved, or it is gone. A move that fails (the old copy is locked, access denied) returns false, so a log that cannot rotate stops growing.
fn make_room(path: &Path, entry: u64) -> bool {
    match std::fs::metadata(path) {
        Ok(m) if !room_for(m.len(), entry) => std::fs::rename(path, path.with_extension("old.log")).is_ok(),
        _ => true,
    }
}

/// Appends a timestamped entry as one write, moving the file aside first when the entry would take it past the cap. Input: the log path and the text. Returns nothing; failures drop the entry, as a log must not fail its caller or grow without bound.
pub fn append(path: &Path, text: &str) {
    let at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let entry = frame(at, text);
    let _writing = WRITING.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if !make_room(path, entry.len() as u64) {
        return;
    }
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = file.write_all(entry.as_bytes());
    }
}

#[cfg(test)]
mod tests;
