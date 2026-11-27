//! File: log/tests.rs
//! Purpose: Protect the log's bounds: an entry never exceeds its cap, a full log is moved aside, and a log that cannot be moved stops growing, and the log lines the harnesses wait for are ones the app writes.
//! Contents: tests of `frame`, `room_for`, `append` and `stage_line`; sources and the check that every log line a harness parses is written by the app.

use super::{append, frame, room_for, stage_line, MAX_ENTRY, MAX_LOG};

/// A scratch folder removed when dropped, so the tests leave nothing in the temp folder.
struct Scratch(std::path::PathBuf);

impl Scratch {
    /// Makes a fresh folder. Input: a label. Returns the guard.
    fn new(label: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("cue-log-test-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }
}

impl Drop for Scratch {
    /// Removes the folder. Returns nothing.
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The whole entry, framing included, is at most the cap, for text of exactly the cap.
#[test]
fn an_entry_never_exceeds_the_cap() {
    let entry = frame(1_700_000_000, &"a".repeat(MAX_ENTRY));
    assert_eq!(entry.len(), MAX_ENTRY);
    assert!(entry.ends_with("a\n"));
}

/// A cut never splits a multi-byte character.
#[test]
fn the_cut_keeps_whole_characters() {
    let entry = frame(1, &"é".repeat(MAX_ENTRY));
    assert!(entry.len() <= MAX_ENTRY);
    assert!(entry.is_char_boundary(entry.len() - 1));
}

/// An entry that lands exactly on the cap fits; one byte more does not.
#[test]
fn room_for_counts_the_boundary() {
    assert!(room_for(MAX_LOG - 10, 10));
    assert!(!room_for(MAX_LOG - 10, 11));
}

/// A full log is moved to the old copy and the new entry starts a fresh file.
#[test]
fn a_full_log_is_moved_aside() {
    let dir = Scratch::new("rotate");
    let log = dir.0.join("cue.log");
    std::fs::write(&log, vec![b'x'; MAX_LOG as usize]).unwrap();
    append(&log, "fresh");
    assert_eq!(std::fs::metadata(dir.0.join("cue.old.log")).unwrap().len(), MAX_LOG);
    assert!(std::fs::read_to_string(&log).unwrap().ends_with("fresh\n"));
}

/// When the old copy cannot be replaced, the log is not written: it stays at its size.
#[test]
fn a_log_that_cannot_rotate_stops_growing() {
    let dir = Scratch::new("stuck");
    let log = dir.0.join("cue.log");
    std::fs::create_dir(dir.0.join("cue.old.log")).unwrap();
    std::fs::write(&log, vec![b'x'; MAX_LOG as usize]).unwrap();
    append(&log, "dropped");
    assert_eq!(std::fs::metadata(&log).unwrap().len(), MAX_LOG);
}

/// A stage line names the stage, the time and the thread, so a stall can be placed between two of them.
#[test]
fn a_stage_line_names_stage_time_and_thread() {
    assert_eq!(stage_line("tray built", 42, "ThreadId(1) main"), "stage: tray built at 42 ms on ThreadId(1) main");
}

/// A reader that holds the log without sharing for a few milliseconds does not make an entry vanish.
#[cfg(windows)]
#[test]
fn an_entry_waits_out_a_reader_that_holds_the_log() {
    use std::os::windows::fs::OpenOptionsExt;
    let dir = Scratch::new("locked");
    let path = dir.0.join("cue.log");
    std::fs::write(&path, "first\n").unwrap();
    let held = std::fs::OpenOptions::new().read(true).share_mode(0).open(&path).unwrap();
    let release = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(10));
        drop(held);
    });
    append(&path, "second");
    release.join().unwrap();
    assert!(std::fs::read_to_string(&path).unwrap().contains("second"));
}

/// Appends the text of every Rust file under a folder, test files left out. Inputs: the folder, the text to append to.
fn sources(dir: &std::path::Path, text: &mut String) {
    for entry in std::fs::read_dir(dir).unwrap().map(Result::unwrap) {
        let path = entry.path();
        if path.is_dir() {
            sources(&path, text);
        } else if path.extension().is_some_and(|e| e == "rs") && path.file_name().is_some_and(|n| n != "tests.rs") {
            text.push_str(&std::fs::read_to_string(&path).unwrap());
        }
    }
}

/// Every log line a harness parses (`scripts/lib/log-lines.json`, `{}` for a number) is one the app writes: a `stage: <name> at {} ms` line names a stage the source passes to `log::stage` and matches `stage_line`, any other is a string or format literal in the source. A renamed line fails here instead of timing out in a Windows run.
#[test]
fn harness_log_lines_are_written_by_the_app() {
    let lines: serde_json::Value = serde_json::from_str(include_str!("../../../scripts/lib/log-lines.json")).unwrap();
    let mut source = String::new();
    sources(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut source);
    for (name, template) in lines.as_object().unwrap() {
        let template = template.as_str().unwrap();
        let written = match template.strip_prefix("stage: ").and_then(|t| t.strip_suffix(" at {} ms")) {
            Some(stage) => {
                source.contains(&format!("stage(\"{stage}\")")) && stage_line(stage, 7, "t").starts_with(&template.replace("{}", "7"))
            }
            None => source.contains(&format!("\"{template}\"")),
        };
        assert!(written, "{name}: no Rust source writes {template:?}");
    }
}
