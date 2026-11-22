//! File: dictionary/catalog/tests.rs
//! Purpose: Protect the gate between reading a pack and changing it, on real temporary SQLite files: a read in progress holds a change back, a read held back by a change sees the new pack, a failed change leaves the old pack usable, and the kept book is released before the files move.
//! Contents: a pack builder; one test per rule.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use rusqlite::Connection;

use super::{changing, reading, swap_in, DATABASE};
use crate::dictionary::book::Book;

/// Writes a pack folder holding `dictionary.db` with one word. Input: the folder and the word.
fn pack_in(dir: &Path, word: &str) {
    std::fs::create_dir_all(dir).expect("folder");
    let conn = Connection::open(dir.join(DATABASE)).expect("create");
    conn.execute_batch("CREATE TABLE meta (k TEXT PRIMARY KEY, v TEXT); INSERT INTO meta VALUES ('language', 'English'); CREATE TABLE entries (key TEXT PRIMARY KEY, data TEXT NOT NULL);")
        .expect("schema");
    let data = format!(r#"[{{"w":"{word}","p":"noun","s":[{{"g":"meaning of {word}"}}]}}]"#);
    conn.execute("INSERT INTO entries VALUES (?1, ?2)", [word, data.as_str()]).expect("entry");
}

/// The words the pack at `target` finds for "w". Input: the pack folder. Return: their keys.
fn found(target: &Path) -> Vec<String> {
    let path: PathBuf = target.join(DATABASE);
    Book::new(vec![("en", "English", path)]).search("w", 5).expect("search").into_iter().map(|h| h.key).collect()
}

/// A change waits for a read in progress and goes ahead once it ends.
#[test]
fn a_read_in_progress_holds_a_change_back() {
    let read = reading();
    let done = Arc::new(AtomicBool::new(false));
    let flag = done.clone();
    let change = std::thread::spawn(move || changing(|| {}, || flag.store(true, Ordering::SeqCst)));
    std::thread::sleep(Duration::from_millis(150));
    assert!(!done.load(Ordering::SeqCst));
    drop(read);
    change.join().expect("change");
    assert!(done.load(Ordering::SeqCst));
}

/// A read that arrives while a change runs waits for it and then opens the new pack.
#[test]
fn a_read_held_back_by_a_change_sees_the_new_pack() {
    let root = tempfile::tempdir().expect("dir");
    let (fresh, target) = (root.path().join("fresh"), root.path().join("pack"));
    pack_in(&target, "wold");
    pack_in(&fresh, "wnew");
    let (inside, entered) = mpsc::channel();
    let (target2, fresh2) = (target.clone(), fresh.clone());
    let change = std::thread::spawn(move || {
        changing(
            || {},
            || {
                inside.send(()).expect("signal");
                std::thread::sleep(Duration::from_millis(150));
                swap_in(&fresh2, &target2)
            },
        )
    });
    entered.recv().expect("the change holds the gate");
    let _read = reading();
    assert_eq!(found(&target), ["wnew"]);
    assert_eq!(change.join().expect("change"), Ok(()));
}

/// A replacement without a database fails and the old pack still answers.
#[test]
fn a_failed_replacement_leaves_the_old_pack_usable() {
    let root = tempfile::tempdir().expect("dir");
    let (fresh, target) = (root.path().join("fresh"), root.path().join("pack"));
    pack_in(&target, "wold");
    std::fs::create_dir_all(&fresh).expect("empty download");
    assert!(changing(|| {}, || swap_in(&fresh, &target)).is_err());
    assert_eq!(found(&target), ["wold"]);
}

/// The kept readers are dropped before the change touches the files.
#[test]
fn the_release_runs_before_the_change() {
    let order = Mutex::new(Vec::new());
    changing(|| order.lock().expect("order").push("release"), || order.lock().expect("order").push("change"));
    assert_eq!(*order.lock().expect("order"), ["release", "change"]);
}
