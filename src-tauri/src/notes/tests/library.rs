//! File: notes/tests/library.rs
//! Purpose: Protect the note library through the page's one command: writing and moving between notes, the empty note that never lingers, trash and restore, pins, search, expiry, and that nothing is lost to a stale or oversized save.
//! Contents: scenario tests of `run` over a real SQLite file.

use rusqlite::Connection;

use crate::notes::store::{self, MAX_BODY};
use crate::notes::{run, Cmd, Move, Note, Reply};

/// A fresh notes database in a temp folder. Returns the folder (keep it alive) and the connection.
fn open_db() -> (tempfile::TempDir, Connection) {
    let dir = tempfile::tempdir().expect("temp dir");
    let conn = store::open(&dir.path().join("notes.db")).expect("open");
    (dir, conn)
}

/// Waits until the millisecond clock moves on, so two notes made one after the other are told apart by their order (notes are ordered by creation time in milliseconds).
fn next_millisecond() {
    let start = store::now();
    while store::now() == start {
        std::thread::yield_now();
    }
}

/// The note on screen after a command. Inputs: the connection and the command. Returns the note, panicking if the reply was something else.
fn shown(conn: &Connection, cmd: Cmd) -> Note {
    match run(conn, cmd).expect("command works") {
        Reply::Open(open) => open.note,
        _ => panic!("the command did not open a note"),
    }
}

/// A new note with some text, made after the previous one. Inputs: the connection and the text. Returns the note.
fn written(conn: &Connection, body: &str) -> Note {
    next_millisecond();
    let note = store::create(conn).expect("create");
    run(conn, Cmd::Save { id: note.id, body: body.into() }).expect("save");
    next_millisecond();
    store::get(conn, note.id).expect("get").expect("exists")
}

/// The rows of the list view as JSON. Inputs: the connection, the search text and whether to read the trash. Returns the rows.
fn rows(conn: &Connection, query: &str, trash: bool) -> Vec<serde_json::Value> {
    match run(conn, Cmd::List { query: query.into(), trash }).expect("list") {
        Reply::Rows(rows) => rows.iter().map(|r| serde_json::to_value(r).expect("row")).collect(),
        _ => panic!("not a list"),
    }
}

/// The first line of each row, in order. Input: the rows. Returns the lines.
fn firsts(rows: &[serde_json::Value]) -> Vec<&str> {
    rows.iter().map(|r| r["first"].as_str().expect("first line")).collect()
}

/// Moving on from a written note starts a new one, an empty note never starts another, and leaving an empty note drops it so blank notes do not pile up.
#[test]
fn moving_between_notes_never_leaves_blank_notes_behind() {
    let (_dir, conn) = open_db();
    let groceries = written(&conn, "Groceries");
    let blank = shown(&conn, Cmd::Go { from: groceries.id, to: Move::New });
    assert_ne!(blank.id, groceries.id);
    next_millisecond();
    assert_eq!(shown(&conn, Cmd::Go { from: blank.id, to: Move::New }).id, blank.id, "an empty note does not start another");
    let back = shown(&conn, Cmd::Go { from: blank.id, to: Move::Older });
    assert_eq!((back.id, back.body.as_str()), (groceries.id, "Groceries"));
    assert!(store::get(&conn, blank.id).expect("get").is_none(), "the empty note is gone for good");
}

/// Older and newer walk the order, stop at both ends, and Newer past the newest note starts a fresh one.
#[test]
fn older_and_newer_walk_the_order_and_stop_at_the_ends() {
    let (_dir, conn) = open_db();
    let (one, two) = (written(&conn, "one"), written(&conn, "two"));
    assert_eq!(shown(&conn, Cmd::Go { from: two.id, to: Move::Older }).id, one.id);
    assert_eq!(shown(&conn, Cmd::Go { from: one.id, to: Move::Older }).id, one.id, "nothing older");
    assert_eq!(shown(&conn, Cmd::Go { from: one.id, to: Move::Newer }).id, two.id);
    let fresh = shown(&conn, Cmd::Go { from: two.id, to: Move::Newer });
    assert!(fresh.id != two.id && fresh.body.is_empty());
    assert_eq!(shown(&conn, Cmd::Go { from: one.id, to: Move::Newest }).id, fresh.id);
}

/// A trashed note leaves the list and shows the newest note; it lives in the trash until restored, and restoring brings it back as the newest.
#[test]
fn trash_and_restore_move_a_note_out_and_back() {
    let (_dir, conn) = open_db();
    let (one, two) = (written(&conn, "one"), written(&conn, "two"));
    assert_eq!(shown(&conn, Cmd::Trash { id: two.id }).id, one.id);
    assert_eq!(firsts(&rows(&conn, "", false)), ["one"]);
    assert_eq!(firsts(&rows(&conn, "", true)), ["two"]);
    next_millisecond();
    assert_eq!(shown(&conn, Cmd::Restore { id: two.id }).id, two.id);
    assert_eq!(shown(&conn, Cmd::Go { from: one.id, to: Move::Newest }).id, two.id);
    assert!(rows(&conn, "", true).is_empty());
}

/// Pinned notes list first; promoting an old note makes it the newest without changing its text.
#[test]
fn pinned_notes_list_first_and_promote_makes_a_note_the_newest() {
    let (_dir, conn) = open_db();
    let (old, newer) = (written(&conn, "old"), written(&conn, "newer"));
    assert_eq!(firsts(&rows(&conn, "", false)), ["newer", "old"]);
    run(&conn, Cmd::Pin { id: old.id, pinned: true }).expect("pin");
    assert_eq!(firsts(&rows(&conn, "", false)), ["old", "newer"]);
    assert_eq!(rows(&conn, "", false)[0]["pinned"], true);
    run(&conn, Cmd::Pin { id: old.id, pinned: false }).expect("unpin");
    assert_eq!(shown(&conn, Cmd::Promote { id: old.id }).body, "old");
    assert_eq!(firsts(&rows(&conn, "", false)), ["old", "newer"]);
    assert_eq!(shown(&conn, Cmd::Go { from: newer.id, to: Move::Newest }).id, old.id);
}

/// Search matches anywhere in a note without case, and `%`, `_` and `\` in the search text mean themselves.
#[test]
fn search_is_case_insensitive_and_wildcards_mean_themselves() {
    let (_dir, conn) = open_db();
    for text in ["100% sure", "a_b path", "plain note", "C:\\temp"] {
        written(&conn, text);
    }
    assert_eq!(firsts(&rows(&conn, "SURE", false)), ["100% sure"]);
    assert_eq!(firsts(&rows(&conn, "100%", false)), ["100% sure"]);
    assert_eq!(firsts(&rows(&conn, "a_b", false)), ["a_b path"]);
    assert_eq!(firsts(&rows(&conn, "_", false)), ["a_b path"]);
    assert_eq!(firsts(&rows(&conn, "\\", false)), ["C:\\temp"]);
    assert_eq!(rows(&conn, "", false).len(), 4);
}

/// Old unpinned notes go to the trash after the chosen days, pinned ones stay, and the trash empties after thirty days; zero days keeps everything.
#[test]
fn sweeping_expires_old_notes_and_empties_an_old_trash() {
    let (_dir, conn) = open_db();
    let day = 86_400_000_i64;
    let ids: Vec<i64> =
        ["recent", "stale", "pinned and stale", "long trashed", "newly trashed"].iter().map(|t| written(&conn, t).id).collect();
    let age = |id: i64, updated_days: i64, trashed_days: Option<i64>| {
        let (updated, trashed) = (store::now() - updated_days * day, trashed_days.map(|d| store::now() - d * day));
        conn.execute("UPDATE notes SET updated = ?2, deleted = ?3 WHERE id = ?1", rusqlite::params![id, updated, trashed]).expect("age");
    };
    age(ids[0], 1, None);
    age(ids[1], 40, None);
    age(ids[2], 60, None);
    age(ids[3], 90, Some(31));
    age(ids[4], 90, Some(5));
    run(&conn, Cmd::Pin { id: ids[2], pinned: true }).expect("pin");

    store::sweep(&conn, 0).expect("keep everything");
    assert_eq!(rows(&conn, "", false).len(), 3);
    store::sweep(&conn, 30).expect("sweep");
    assert_eq!(firsts(&rows(&conn, "", false)), ["pinned and stale", "recent"]);
    let trash = rows(&conn, "", true);
    assert_eq!(firsts(&trash), ["stale", "newly trashed"]);
}

/// A stale page or an oversized paste cannot corrupt the library: opening a missing note says so, and a note past the limit is refused whole.
#[test]
fn a_missing_note_and_an_oversized_note_are_refused() {
    let (_dir, conn) = open_db();
    let note = written(&conn, "kept");
    assert_eq!(run(&conn, Cmd::Go { from: 9999, to: Move::Older }).err().as_deref(), Some("The note is gone."));
    let too_long = "x".repeat(MAX_BODY + 1);
    assert_eq!(run(&conn, Cmd::Save { id: note.id, body: too_long }).err().as_deref(), Some("The note is too long to save."));
    assert_eq!(store::get(&conn, note.id).expect("get").expect("exists").body, "kept");
}

/// What was typed survives closing and reopening the file (a note is saved as you type).
#[test]
fn notes_survive_reopening_the_database() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("notes.db");
    let id = {
        let conn = store::open(&path).expect("open");
        written(&conn, "line one\nline two").id
    };
    let conn = store::open(&path).expect("reopen");
    assert_eq!(store::get(&conn, id).expect("get").expect("exists").body, "line one\nline two");
}
