//! File: log/tests.rs
//! Purpose: Protect the log's bounds: an entry never exceeds its cap, a full log is moved aside, and a log that cannot be moved stops growing.
//! Contents: tests of `frame`, `room_for` and `append`.

use super::{append, frame, room_for, MAX_ENTRY, MAX_LOG};

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
