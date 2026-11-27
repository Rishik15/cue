//! File: fsutil/tests.rs
//! Purpose: Protect `write_atomic` (it replaces a file whole, leaves no temporary file, and a failed write leaves the old file alone) `commit` (a finished file replaces the target whole; a failed one leaves it) `sync_tree` (every file of a nested folder is synced, a missing folder is an error) and `set_aside` (a recovery copy is never replaced; a failed name lookup is an error, never a hang).
//! Contents: tests of `write_atomic`, `commit`, `sync_tree` and `set_aside`.

use super::{commit, free_copy_name, set_aside, sync_tree, write_atomic, MAX_COPIES};

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

/// A new file is written, its folder created, and no temporary file is left.
#[test]
fn writes_new_file_and_leaves_no_temp() {
    let dir = Scratch::new("new");
    let path = dir.0.join("sub").join("a.json");
    write_atomic(&path, b"one").unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"one");
    let names: Vec<_> = std::fs::read_dir(path.parent().unwrap()).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(names, vec![std::ffi::OsString::from("a.json")]);
}

/// An existing file is replaced by the new bytes.
#[test]
fn replaces_existing_file() {
    let dir = Scratch::new("replace");
    let path = dir.0.join("a.json");
    std::fs::write(&path, b"old").unwrap();
    write_atomic(&path, b"new").unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"new");
}

/// A failed write (a folder where the temporary file goes) leaves the old file and the blocker alone.
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

/// Each damaged file goes to the next free recovery name with its bytes, so no earlier copy is replaced; a file that cannot be moved (here a missing one) is an error and creates no copy.
#[test]
fn set_aside_never_replaces_a_copy() {
    let dir = Scratch::new("aside");
    let path = dir.0.join("rules.json");
    for (n, name) in ["rules.damaged.json", "rules.damaged-2.json", "rules.damaged-3.json"].iter().enumerate() {
        std::fs::write(&path, n.to_string()).unwrap();
        assert_eq!(set_aside(&path).unwrap(), *name);
        assert_eq!(std::fs::read_to_string(dir.0.join(name)).unwrap(), n.to_string());
    }
    assert!(set_aside(&path).is_err());
    assert!(!dir.0.join("rules.damaged-4.json").exists());
}

/// A finished file replaces the target with its bytes and is gone from its old name; a missing one is an error and the target keeps its bytes.
#[test]
fn commit_replaces_target_or_leaves_it() {
    let dir = Scratch::new("commit");
    let (part, target) = (dir.0.join("m.bin.part"), dir.0.join("m.bin"));
    std::fs::write(&target, b"old").unwrap();
    std::fs::write(&part, b"new").unwrap();
    commit(&part, &target).unwrap();
    assert_eq!(std::fs::read(&target).unwrap(), b"new");
    assert!(!part.exists());
    assert!(commit(&part, &target).is_err());
    assert_eq!(std::fs::read(&target).unwrap(), b"new");
}

/// A name lookup that keeps failing is an error at once, every name taken is an error after `MAX_COPIES` tries, and the damaged file stays in place.
#[test]
fn recovery_name_lookup_fails_instead_of_looping() {
    let dir = Scratch::new("lookup");
    let path = dir.0.join("settings.json");
    std::fs::write(&path, b"damaged").unwrap();
    let tries = std::cell::Cell::new(0);
    let denied = |_: &std::path::Path| {
        tries.set(tries.get() + 1);
        Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
    };
    assert_eq!(free_copy_name(&path, denied).unwrap_err().kind(), std::io::ErrorKind::PermissionDenied);
    assert_eq!(tries.get(), 1);
    tries.set(0);
    let taken = |_: &std::path::Path| {
        tries.set(tries.get() + 1);
        Ok(true)
    };
    assert!(free_copy_name(&path, taken).is_err());
    assert_eq!(tries.get(), MAX_COPIES);
    assert_eq!(std::fs::read(&path).unwrap(), b"damaged");
}

/// Every file of a nested folder is synced and left unchanged; a missing folder is an error, so a publish never renames a folder it could not sync.
#[test]
fn sync_tree_syncs_nested_files_and_fails_on_a_missing_folder() {
    let dir = Scratch::new("sync-tree");
    std::fs::create_dir_all(dir.0.join("a/b")).unwrap();
    std::fs::write(dir.0.join("top.bin"), b"top").unwrap();
    std::fs::write(dir.0.join("a/b/deep.bin"), b"deep").unwrap();
    sync_tree(&dir.0).unwrap();
    assert_eq!(std::fs::read(dir.0.join("a/b/deep.bin")).unwrap(), b"deep");
    assert!(sync_tree(&dir.0.join("missing")).is_err());
}
