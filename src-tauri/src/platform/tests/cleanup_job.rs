//! File: platform/tests/cleanup_job.rs
//! Purpose: Tests of the remove-data command line and of deleting folders whose names a shell would mangle.
//! Contents: the `Scratch` folder guard and its test; parse accepts and rejects; remove_all on `%X%` names, on a missing folder, and (Windows) on a folder with a file held open.

use crate::platform::cleanup_job::{parse, remove_all};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Builds the argument list a helper process receives.
fn args(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|s| s.to_string()).collect()
}

/// A fresh empty folder under the temp directory, deleted when dropped so a failed or held-file test leaves nothing behind.
struct Scratch(PathBuf);

impl Scratch {
    /// Makes `cue-cleanup-<name>-<pid>` empty. Input: the test's name. Returns the guard.
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("cue-cleanup-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    /// Joins a path under the folder. Input: the relative part. Returns the full path.
    fn join(&self, part: impl AsRef<Path>) -> PathBuf {
        self.0.join(part)
    }
}

impl Drop for Scratch {
    /// Deletes the folder and everything in it.
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Dropping the guard removes the folder and its contents.
#[test]
fn scratch_is_removed_on_drop() {
    let base = Scratch::new("drop");
    std::fs::create_dir_all(base.join("inner")).unwrap();
    std::fs::write(base.join("inner").join("f.txt"), "x").unwrap();
    let dir = base.0.clone();
    drop(base);
    assert!(!dir.exists());
}

/// The exact command line is accepted and the rest is refused.
#[test]
fn parse_accepts_only_the_exact_form() {
    let (a, b) = (std::env::temp_dir().join("a"), std::env::temp_dir().join("b"));
    let (a, b) = (a.to_str().unwrap(), b.to_str().unwrap());
    let job = parse(&args(&["cue.exe", "--remove-data", "42", a, b])).unwrap();
    assert_eq!((job.pid, job.dirs.len()), (42, 2));
    let root = std::env::temp_dir().ancestors().last().unwrap().to_str().unwrap().to_string();
    for bad in [
        args(&["cue.exe", "--remove-data"]),
        args(&["cue.exe", "--remove-data", "x", a]),
        args(&["cue.exe", "--remove-data", "42"]),
        args(&["cue.exe", "--remove-data", "42", "relative"]),
        args(&["cue.exe", "--remove-data", "42", &root]),
        args(&["cue.exe", "--other", "42", a]),
    ] {
        assert!(parse(&bad).is_none(), "{bad:?}");
    }
}

/// Folders named with `%X%`, spaces, `&`, `^` and unicode go; their lookalike neighbours stay.
#[test]
fn remove_all_takes_only_the_named_folders() {
    let base = Scratch::new("names");
    let names = ["%X%", "a b", "x&y", "c^d", "naïve-日本"];
    let dirs: Vec<PathBuf> = names.iter().map(|n| base.join(n)).collect();
    for d in &dirs {
        std::fs::create_dir_all(d.join("inner")).unwrap();
        std::fs::write(d.join("inner").join("f.txt"), "x").unwrap();
    }
    std::fs::create_dir_all(base.join("X")).unwrap();
    std::fs::write(base.join("%X%.txt"), "x").unwrap();
    let left = remove_all(&dirs, Duration::from_millis(10), Duration::from_secs(1));
    assert!(left.is_empty());
    assert!(dirs.iter().all(|d| !d.exists()));
    assert!(base.join("X").is_dir() && base.join("%X%.txt").is_file());
}

/// An apostrophe in a folder name survives the command line and the delete (nothing parses it as a quote).
#[test]
fn apostrophe_path_round_trips_through_argv() {
    let base = Scratch::new("apostrophe");
    let dir = base.join("O'Brien's data");
    std::fs::create_dir_all(dir.join("inner")).unwrap();
    let job = parse(&args(&["cue.exe", "--remove-data", "7", dir.to_str().unwrap()])).unwrap();
    assert_eq!(job.dirs, vec![dir.clone()]);
    assert!(remove_all(&job.dirs, Duration::from_millis(10), Duration::from_secs(1)).is_empty());
    assert!(!dir.exists());
}

/// A folder that is already gone is success.
#[test]
fn remove_all_missing_folder_is_done() {
    let base = Scratch::new("missing");
    let gone = base.join("nope");
    assert!(remove_all(&[gone], Duration::from_millis(10), Duration::from_millis(50)).is_empty());
}

/// A file held open (Windows refuses to delete it) keeps its folder, which is reported after the give-up time.
#[cfg(windows)]
#[test]
fn remove_all_reports_a_folder_with_a_held_file() {
    let base = Scratch::new("held");
    let dir = base.join("data");
    std::fs::create_dir_all(&dir).unwrap();
    use std::os::windows::fs::OpenOptionsExt;
    let held = std::fs::OpenOptions::new().write(true).create(true).truncate(true).share_mode(0).open(dir.join("held.txt")).unwrap();
    let left = remove_all(std::slice::from_ref(&dir), Duration::from_millis(50), Duration::from_millis(300));
    assert_eq!(left, vec![dir]);
    drop(held);
}
