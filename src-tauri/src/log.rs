//! File: log.rs
//! Purpose: Cue's activity log: one line per notable event (a grab and what it read, every problem shown to the user) in `cue.log` in the data folder, next to `crash.log`, so what Cue did can be read afterwards.
//! Contents: init — where the log lives; note — append one line; append — the shared writer (also used for `crash.log`): timestamped, cut to a line cap, written in one call, and moved aside past a size cap.
//! Callers pass events and error text only, never what the user typed, said or named.
//! Nothing here runs while idle: a line is written only when something happens.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use tauri::{AppHandle, Manager};

static LOG: OnceLock<PathBuf> = OnceLock::new();
/// A log past this size is moved aside (one old copy is kept), so a loop of events cannot fill the disk.
const MAX_LOG: u64 = 256 * 1024;
/// One entry never exceeds this many bytes, so the file stays near the cap whatever a message holds (a crash backtrace fits).
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

/// Appends a timestamped entry as one write, moving the file aside first when the entry would take it past the cap. Input: the log path and the text, cut to `MAX_ENTRY` bytes. Returns nothing; failures are dropped, as a log must not fail its caller.
pub fn append(path: &Path, text: &str) {
    let at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let mut end = text.len().min(MAX_ENTRY);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let entry = format!("[{at}] {}\n", &text[..end]);
    if std::fs::metadata(path).is_ok_and(|m| m.len() + entry.len() as u64 > MAX_LOG) {
        let _ = std::fs::rename(path, path.with_extension("old.log"));
    }
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = file.write_all(entry.as_bytes());
    }
}
