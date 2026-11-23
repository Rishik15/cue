//! File: fsutil/tests.rs
//! Purpose: Protect `write_atomic`: it replaces a file whole, leaves no temporary file, and a failed write leaves the old file alone.
//! Contents: tests of `write_atomic`.

use super::write_atomic;

/// A scratch folder removed when dropped. Input: a label. Returns the guard holding the folder's path.
struct Scratch(std::path::PathBuf);

impl Scratch {
    /// Makes a fresh folder. Input: a label. Returns the guard.
    fn new(label: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("cue-fsutil-test-{label}-{}", std::process::id()));
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

#[test]
fn writes_new_file_and_leaves_no_temp() {
    let dir = Scratch::new("new");
    let path = dir.0.join("sub").join("a.json");
    write_atomic(&path, b"one").unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"one");
    let names: Vec<_> = std::fs::read_dir(path.parent().unwrap()).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(names, vec![std::ffi::OsString::from("a.json")]);
}

#[test]
fn replaces_existing_file() {
    let dir = Scratch::new("replace");
    let path = dir.0.join("a.json");
    std::fs::write(&path, b"old").unwrap();
    write_atomic(&path, b"new").unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"new");
}

#[test]
fn failed_write_keeps_old_file_and_blocker() {
    let dir = Scratch::new("blocked");
    let path = dir.0.join("a.json");
    std::fs::write(&path, b"old").unwrap();
    let blocker = dir.0.join("a.json.tmp");
    std::fs::create_dir(&blocker).unwrap();
    assert!(write_atomic(&path, b"new").is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"old");
    assert!(blocker.is_dir());
}
