//! File: log.rs
//! Purpose: Cue's activity log: one line per notable event (a grab and what it read, every problem shown to the user) in `cue.log` in the data folder, next to `crash.log`, so what Cue did can be read afterwards.
//! Contents: init — where the log lives; note — append one line; stage — a startup stage line with its time and thread; write_entry — the open-and-retry write; append — the shared writer (also used for `crash.log`): timestamped, cut so the whole entry fits the entry cap, written in one call, and moved aside past a size cap; frame / room_for — pure entry and rotation arithmetic.
//! Callers pass events and error text only, never what the user typed, said or named.
//! Nothing here runs while idle: a line is written only when something happens.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use tauri::{AppHandle, Manager};

static LOG: OnceLock<PathBuf> = OnceLock::new();
/// When the log was set up, the zero of the stage timestamps.
static BEGAN: OnceLock<Instant> = OnceLock::new();
/// Held from the size check to the end of the write, so two threads cannot rotate the same file over each other's backup.
static WRITING: Mutex<()> = Mutex::new(());
/// A log past this size is moved aside (one old copy is kept), so a loop of events cannot fill the disk.
const MAX_LOG: u64 = 256 * 1024;
/// How many times a write tries to open the log, and the pause between tries; about 40 ms in all, so a writer blocked by a reader waits it out and a log that stays locked still gives up quickly.
const OPEN_TRIES: u32 = 20;
const OPEN_PAUSE: std::time::Duration = std::time::Duration::from_millis(2);
/// One whole entry, timestamp and line end included, never exceeds this many bytes, so the file stays near the cap whatever a message holds (a crash backtrace fits).
const MAX_ENTRY: usize = 8 * 1024;

/// Remembers where `cue.log` lives, creating the data folder. Input: the app handle. Returns nothing; without a data folder `note` does nothing.
pub fn init(app: &AppHandle) {
    let _ = BEGAN.set(Instant::now());
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

/// One startup stage line: the stage, the milliseconds since the log began and the thread it ran on.
fn stage_line(stage: &str, ms: u128, thread: &str) -> String {
    format!("stage: {stage} at {ms} ms on {thread}")
}

/// Appends a startup stage line with its time since `init` and its thread, so a stall can be placed between two stages. Input: the stage name. Returns nothing.
pub fn stage(stage: &str) {
    let ms = BEGAN.get().map_or(0, |began| began.elapsed().as_millis());
    let thread = std::thread::current();
    note(&stage_line(stage, ms, &format!("{:?} {}", thread.id(), thread.name().unwrap_or("unnamed"))));
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
    write_entry(path, &entry);
}

/// Appends `entry` to the file at `path`, trying a few times when the open fails: a reader that holds the log without sharing (a viewer, a scanner, a script) must not make an entry vanish. Inputs: the log path and the framed entry. Returns nothing; after the last try the entry is dropped.
fn write_entry(path: &Path, entry: &str) {
    for attempt in 0..OPEN_TRIES {
        if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
            let _ = file.write_all(entry.as_bytes());
            return;
        }
        if attempt + 1 < OPEN_TRIES {
            std::thread::sleep(OPEN_PAUSE);
        }
    }
}

#[cfg(test)]
mod tests;
