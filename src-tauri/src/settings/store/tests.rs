//! File: settings/store/tests.rs
//! Purpose: Protect the settings state: a change reaches the disk before readers see it, a refused or failed change leaves both untouched, and two writers never undo each other.
//! Contents: tests of `Store::open`, `update`, `snapshot` and `refusal`.

use std::sync::Arc;

use serde_json::json;

use super::Store;
use crate::settings::file::tests::Scratch;
use crate::settings::file::{load, VERSION};

/// A saved change is in the snapshot and in the file, with the version stamp.
#[test]
fn a_saved_change_is_visible_and_durable() {
    let dir = Scratch::new("store-save");
    let (store, notice) = Store::open(dir.file());
    assert!(notice.is_none());
    store.update(&[("theme_mode", Some(json!("Dark")))]).unwrap();
    assert_eq!(store.snapshot()["theme_mode"], "Dark");
    let on_disk = load(&dir.file()).map;
    assert_eq!(on_disk["theme_mode"], "Dark");
    assert_eq!(on_disk["settings_version"], VERSION);
}

/// Several keys change in one save; None removes a key.
#[test]
fn keys_change_together_and_can_be_removed() {
    let dir = Scratch::new("store-multi");
    let (store, _) = Store::open(dir.file());
    store.update(&[("a", Some(json!(1))), ("b", Some(json!(2)))]).unwrap();
    store.update(&[("a", None), ("c", Some(json!(3)))]).unwrap();
    let map = load(&dir.file()).map;
    assert!(!map.contains_key("a") && map["b"] == 2 && map["c"] == 3);
}

/// A snapshot taken earlier does not change when the settings do.
#[test]
fn a_snapshot_stays_as_it_was() {
    let dir = Scratch::new("store-snap");
    let (store, _) = Store::open(dir.file());
    store.update(&[("a", Some(json!(1)))]).unwrap();
    let before = store.snapshot();
    store.update(&[("a", Some(json!(2)))]).unwrap();
    assert_eq!(before["a"], 1);
    assert_eq!(store.snapshot()["a"], 2);
    assert!(!Arc::ptr_eq(&before, &store.snapshot()));
}

/// A failed save leaves the snapshot and the next save free of the refused value.
#[test]
fn a_failed_save_changes_nothing() {
    let dir = Scratch::new("store-fail");
    let (store, _) = Store::open(dir.file());
    store.update(&[("a", Some(json!(1)))]).unwrap();
    std::fs::remove_file(dir.file()).unwrap();
    std::fs::create_dir(dir.file()).unwrap();
    assert!(store.update(&[("a", Some(json!(99)))]).is_err());
    assert_eq!(store.snapshot()["a"], 1);
    std::fs::remove_dir(dir.file()).unwrap();
    store.update(&[("b", Some(json!(2)))]).unwrap();
    let map = load(&dir.file()).map;
    assert!(map["a"] == 1 && map["b"] == 2, "the refused value never reaches the disk");
}

/// A newer file is readable, refuses every change and stays byte for byte as it was.
#[test]
fn a_newer_file_refuses_changes_and_is_untouched() {
    let dir = Scratch::new("store-newer");
    let content = format!(r#"{{"settings_version":{},"x":1}}"#, VERSION + 1);
    std::fs::write(dir.file(), &content).unwrap();
    let (store, _) = Store::open(dir.file());
    assert!(store.is_newer());
    assert_eq!(store.snapshot()["x"], 1);
    assert_eq!(store.update(&[("x", Some(json!(2)))]), Err(crate::settings::file::NEWER.to_string()));
    assert_eq!(std::fs::read_to_string(dir.file()).unwrap(), content);
}

/// A blocked file refuses changes with its own text, and the user is told at open.
#[test]
fn a_blocked_file_refuses_changes() {
    let dir = Scratch::new("store-blocked");
    std::fs::create_dir(dir.file()).unwrap();
    let (store, notice) = Store::open(dir.file());
    assert!(notice.is_some());
    assert_eq!(store.refusal(), notice);
    assert!(store.update(&[("a", Some(json!(1)))]).is_err());
}

/// A change that alters nothing does not rewrite the file.
#[test]
fn an_unchanged_value_writes_nothing() {
    let dir = Scratch::new("store-noop");
    let (store, _) = Store::open(dir.file());
    store.update(&[("a", Some(json!(1)))]).unwrap();
    std::fs::write(dir.file(), "sentinel").unwrap();
    store.update(&[("a", Some(json!(1)))]).unwrap();
    assert_eq!(std::fs::read_to_string(dir.file()).unwrap(), "sentinel");
}

/// Many threads changing different keys all land: no write undoes another.
#[test]
fn concurrent_writers_do_not_undo_each_other() {
    let dir = Scratch::new("store-race");
    let (store, _) = Store::open(dir.file());
    let store = Arc::new(store);
    let threads: Vec<_> = (0..8)
        .map(|i| {
            let store = Arc::clone(&store);
            std::thread::spawn(move || store.update(&[(format!("k{i}").as_str(), Some(json!(i)))]).unwrap())
        })
        .collect();
    threads.into_iter().for_each(|t| t.join().unwrap());
    let map = load(&dir.file()).map;
    assert!((0..8).all(|i| map[&format!("k{i}")] == i));
    assert_eq!(store.snapshot().len(), map.len());
}
