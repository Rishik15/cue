//! File: dictionary/catalog/tests.rs
//! Purpose: Protect the gate between reading a pack and changing it, on real temporary SQLite files: a read in progress holds a change back, a read after a change (none can enter during it) sees the new pack, a failed change leaves the old pack usable, and the Search window's kept book (a real open connection in `window::BOOK`) is dropped by the production `changed` before the files are replaced or deleted.
//! Contents: a pack builder; one test per rule. The contention tests prove exclusion with non-blocking try-locks on the gate while the opposing side is held; no threads, no sleeps.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Weak};

use rusqlite::Connection;

use super::{change_blocked, changed, changing, delete, read_blocked, reading, swap_in, DATABASE};
use crate::dictionary::book::Book;
use crate::dictionary::{serial_test, window};

/// The window number the tests keep their book under; the one real slot is shared, so the tests that use it hold `serial_test`.
const WINDOW: u64 = 1_000_000;

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

/// Keeps a book over the pack at `target` in the window's real slot and opens its connection with a search, then lets go of every handle but the slot's. Input: the pack folder. Return: a weak handle that upgrades only while the slot (or a reader) still has the book.
fn keep_open_book(target: &Path) -> Weak<Book> {
    let path = target.join(DATABASE);
    let book = window::kept(WINDOW, || Book::new(vec![("en", "English", path)]));
    assert_eq!(book.search("w", 5).expect("search").len(), 1);
    Arc::downgrade(&book)
}

/// While a read is in progress the gate cannot be taken for a change; once the read ends the change runs.
#[test]
fn a_read_in_progress_holds_a_change_back() {
    let _serial = serial_test();
    let read = reading();
    assert!(change_blocked(), "a change could take the gate during a read");
    drop(read);
    assert!(!change_blocked());
    assert_eq!(changing(|| {}, || "changed"), "changed");
}

/// While a change runs no read can enter, and a read after it opens the new pack.
#[test]
fn a_read_after_a_change_sees_the_new_pack() {
    let _serial = serial_test();
    let root = tempfile::tempdir().expect("dir");
    let (fresh, target) = (root.path().join("fresh"), root.path().join("pack"));
    pack_in(&target, "wold");
    pack_in(&fresh, "wnew");
    let result = changing(
        || {},
        || {
            assert!(read_blocked(), "a read could enter during a change");
            swap_in(&fresh, &target)
        },
    );
    assert_eq!(result, Ok(()));
    assert!(!read_blocked());
    let _read = reading();
    assert_eq!(found(&target), ["wnew"]);
}

/// A replacement without a database fails and the old pack still answers.
#[test]
fn a_failed_replacement_leaves_the_old_pack_usable() {
    let _serial = serial_test();
    let root = tempfile::tempdir().expect("dir");
    let (fresh, target) = (root.path().join("fresh"), root.path().join("pack"));
    pack_in(&target, "wold");
    std::fs::create_dir_all(&fresh).expect("empty download");
    assert!(changing(|| {}, || swap_in(&fresh, &target)).is_err());
    assert_eq!(found(&target), ["wold"]);
}

/// The production `changed` drops the book the Search window kept (a real open connection) before it replaces the pack, and the replacement succeeds.
#[test]
fn a_replaced_pack_is_not_held_open_by_the_kept_book() {
    let _serial = serial_test();
    let root = tempfile::tempdir().expect("dir");
    let (fresh, target) = (root.path().join("fresh"), root.path().join("pack"));
    pack_in(&target, "wold");
    pack_in(&fresh, "wnew");
    let kept = keep_open_book(&target);
    assert!(kept.upgrade().is_some(), "the window keeps the book");
    let result = changed(|| {
        assert!(kept.upgrade().is_none(), "the connection was still open when the files moved");
        swap_in(&fresh, &target)
    });
    assert_eq!(result, Ok(()));
    assert_eq!(found(&target), ["wnew"]);
}

/// The production delete drops the kept book too, and removes the folder.
#[test]
fn a_removed_pack_is_not_held_open_by_the_kept_book() {
    let _serial = serial_test();
    let root = tempfile::tempdir().expect("dir");
    let target = root.path().join("pack");
    pack_in(&target, "wold");
    let kept = keep_open_book(&target);
    assert!(kept.upgrade().is_some(), "the window keeps the book");
    delete(&target).expect("delete");
    assert!(kept.upgrade().is_none(), "the kept book survived the delete");
    assert!(!target.exists());
}

/// The kept readers are dropped before the change touches the files.
#[test]
fn the_release_runs_before_the_change() {
    let _serial = serial_test();
    let order = Mutex::new(Vec::new());
    changing(|| order.lock().expect("order").push("release"), || order.lock().expect("order").push("change"));
    assert_eq!(*order.lock().expect("order"), ["release", "change"]);
}
