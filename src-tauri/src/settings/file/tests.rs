//! File: settings/file/tests.rs
//! Purpose: Protect the settings file: every kind of bad file ends in a safe state, and a write replaces the file whole or not at all.
//! Contents: tests of `load` and `write_atomic`.

use super::{load, write_atomic, Map, Mode, MAX_FILE_BYTES, VERSION};

/// A scratch folder removed when dropped, so the tests leave nothing in the temp folder.
pub struct Scratch(pub std::path::PathBuf);

impl Scratch {
    /// Makes a fresh folder. Input: a label. Returns the guard.
    pub fn new(label: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("cue-settings-test-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }

    /// The settings file's path inside the folder. Returns it.
    pub fn file(&self) -> std::path::PathBuf {
        self.0.join("settings.json")
    }
}

impl Drop for Scratch {
    /// Removes the folder. Returns nothing.
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A file that was never written is fresh defaults and no notice.
#[test]
fn a_missing_file_is_fresh_defaults() {
    let dir = Scratch::new("missing");
    let loaded = load(&dir.file());
    assert_eq!(loaded.mode, Mode::Ready);
    assert!(loaded.map.is_empty() && loaded.notice.is_none());
}

/// A good file is read as it is.
#[test]
fn a_good_file_is_read() {
    let dir = Scratch::new("good");
    std::fs::write(dir.file(), r#"{"theme_mode":"Dark","settings_version":1}"#).unwrap();
    let loaded = load(&dir.file());
    assert_eq!(loaded.mode, Mode::Ready);
    assert_eq!(loaded.map["theme_mode"], "Dark");
}

/// Syntax errors and any JSON that is not an object are set aside with a notice, and the damaged file is kept under its new name.
#[test]
fn a_damaged_file_is_set_aside() {
    for (label, content) in [("syntax", "{not json"), ("array", "[]"), ("null", "null"), ("scalar", "7")] {
        let dir = Scratch::new(label);
        std::fs::write(dir.file(), content).unwrap();
        let loaded = load(&dir.file());
        assert_eq!(loaded.mode, Mode::Ready, "{label}");
        assert!(loaded.notice.is_some() && loaded.map.is_empty(), "{label}");
        assert!(!dir.file().exists(), "{label}");
        assert_eq!(std::fs::read_to_string(dir.0.join("settings.damaged.json")).unwrap(), content, "{label}");
    }
}

/// When a damaged file cannot be moved, it stays, writes are refused and the user is told.
#[test]
fn a_damaged_file_that_cannot_move_blocks_writes() {
    let dir = Scratch::new("stuck");
    std::fs::write(dir.file(), "[]").unwrap();
    std::fs::create_dir(dir.0.join("settings.damaged.json")).unwrap();
    let loaded = load(&dir.file());
    assert!(matches!(loaded.mode, Mode::Blocked(_)) && loaded.notice.is_some());
    assert_eq!(std::fs::read_to_string(dir.file()).unwrap(), "[]");
}

/// A file that cannot be read (here a folder in its place) blocks writes and is not renamed.
#[test]
fn an_unreadable_file_blocks_writes() {
    let dir = Scratch::new("unreadable");
    std::fs::create_dir(dir.file()).unwrap();
    let loaded = load(&dir.file());
    assert!(matches!(loaded.mode, Mode::Blocked(_)));
    assert!(dir.file().is_dir());
}

/// A file over the size cap is left untouched and blocks writes.
#[test]
fn an_oversized_file_is_left_alone() {
    let dir = Scratch::new("big");
    std::fs::write(dir.file(), vec![b' '; MAX_FILE_BYTES as usize + 1]).unwrap();
    let loaded = load(&dir.file());
    assert!(matches!(loaded.mode, Mode::Blocked(_)));
    assert_eq!(std::fs::metadata(dir.file()).unwrap().len(), MAX_FILE_BYTES + 1);
}

/// A file from a newer Cue is read-only, kept byte for byte, with no notice.
#[test]
fn a_newer_file_is_read_only() {
    let dir = Scratch::new("newer");
    let content = format!(r#"{{"settings_version":{},"future":true}}"#, VERSION + 1);
    std::fs::write(dir.file(), &content).unwrap();
    let loaded = load(&dir.file());
    assert_eq!(loaded.mode, Mode::Newer);
    assert_eq!(loaded.map["future"], true);
    assert_eq!(std::fs::read_to_string(dir.file()).unwrap(), content);
}

/// A write replaces the old file whole and leaves no temporary file.
#[test]
fn a_write_replaces_the_file_whole() {
    let dir = Scratch::new("write");
    std::fs::write(dir.file(), r#"{"old":1}"#).unwrap();
    let mut map = Map::new();
    map.insert("new".into(), 2.into());
    write_atomic(&dir.file(), &map).unwrap();
    assert_eq!(load(&dir.file()).map["new"], 2);
    assert!(!load(&dir.file()).map.contains_key("old"));
    assert!(!dir.0.join("settings.json.tmp").exists());
}

/// A failed replace (a folder in the way) leaves the folder, reports the error and leaves no temporary file.
#[test]
fn a_failed_write_cleans_up_and_reports() {
    let dir = Scratch::new("fail");
    std::fs::create_dir(dir.file()).unwrap();
    assert!(write_atomic(&dir.file(), &Map::new()).is_err());
    assert!(!dir.0.join("settings.json.tmp").exists());
    assert!(dir.file().is_dir());
}

/// A map too large to read back is refused before anything is written.
#[test]
fn a_map_too_large_to_read_back_is_refused() {
    let dir = Scratch::new("toolarge");
    let mut map = Map::new();
    map.insert("v".into(), "x".repeat(MAX_FILE_BYTES as usize).into());
    assert!(write_atomic(&dir.file(), &map).is_err());
    assert!(!dir.file().exists());
}

/// An unstamped file loads ready with its keys untouched.
#[test]
fn an_unstamped_file_loads_ready() {
    let dir = Scratch::new("unstamped");
    std::fs::write(dir.file(), r#"{"a":1}"#).unwrap();
    let loaded = load(&dir.file());
    assert_eq!(loaded.mode, Mode::Ready);
    assert_eq!(serde_json::Value::Object(loaded.map), serde_json::json!({"a":1}));
}
