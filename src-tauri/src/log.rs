//! Purpose: Cue's activity log: one line per notable event (a grab and what it read, every problem shown to the user) in `cue.log` in the data folder, next to `crash.log`, so what Cue did can be read afterwards.
//! Contents: init — where the log lives; note — append one line; append — the shared writer (also used for `crash.log`): timestamped, and moved aside past a size cap.
//! Nothing here runs while idle: a line is written only when something happens.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use tauri::{AppHandle, Manager};

static LOG: OnceLock<PathBuf> = OnceLock::new();
/// A log past this size is moved aside (one old copy is kept), so a loop of events cannot fill the disk.
const MAX_LOG: u64 = 256 * 1024;

pub fn init(app: &AppHandle) {
    if let Ok(dir) = app.path().app_data_dir() {
        let _ = std::fs::create_dir_all(&dir);
        let _ = LOG.set(dir.join("cue.log"));
    }
}

pub fn note(line: &str) {
    if let Some(path) = LOG.get() {
        append(path, line);
    }
}

pub fn append(path: &Path, text: &str) {
    if std::fs::metadata(path).is_ok_and(|m| m.len() > MAX_LOG) {
        let _ = std::fs::rename(path, path.with_extension("old.log"));
    }
    let at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "[{at}] {text}");
    }
}
