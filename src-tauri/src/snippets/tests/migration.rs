//! File: snippets/tests/migration.rs
//! Purpose: Protect upgrades of the snippets file: an old database keeps every snippet and trigger, a backup is left beside it, folder prefixes become part of the triggers, and two first opens at once do not both migrate.
//! Contents: tests that build old databases by hand and open them with `store::open`.

use rusqlite::Connection;

use crate::snippets::store;

/// The first version of the file: five columns and no `user_version`.
fn first_version(path: &std::path::Path) {
    let old = Connection::open(path).expect("create");
    old.execute_batch(
        "CREATE TABLE snippets (id INTEGER PRIMARY KEY, name TEXT NOT NULL, keyword TEXT NOT NULL DEFAULT '', body TEXT NOT NULL, uses INTEGER NOT NULL DEFAULT 0);
         INSERT INTO snippets (name, keyword, body, uses) VALUES ('Sig', ':sig', 'Best', 3), ('Mail', ';em', 'a@b.com', 0);",
    )
    .expect("fill");
}

/// The user version of an open database. Input: the connection. Returns it.
fn version(conn: &Connection) -> i64 {
    conn.query_row("PRAGMA user_version", [], |r| r.get(0)).expect("version")
}

/// The first-version file upgrades in place: every snippet and its keyword survive as a trigger, usage counts stay, a backup of the old file is left, and opening again changes nothing.
#[test]
fn an_old_database_upgrades_keeping_every_snippet_and_leaves_a_backup() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("snippets.db");
    first_version(&path);
    let conn = store::open(&path).expect("upgrade");
    let all = store::list(&conn).expect("list");
    assert_eq!(
        all.iter().map(|s| (s.name.as_str(), s.triggers[0].as_str(), s.uses)).collect::<Vec<_>>(),
        [("Sig", ":sig", 3), ("Mail", ";em", 0)]
    );
    assert!(all.iter().all(|s| s.enabled && s.folder_id.is_none()));
    assert_eq!(version(&conn), 3);

    let backup = Connection::open(dir.path().join("snippets.backup.db")).expect("backup exists");
    assert_eq!(version(&backup), 0);
    assert_eq!(backup.query_row("SELECT COUNT(*) FROM snippets", [], |r| r.get::<_, i64>(0)).expect("rows"), 2);

    drop(conn);
    let again = store::open(&path).expect("reopen");
    assert_eq!(store::list(&again).expect("list").len(), 2);
}

/// A folder prefix used to be added while typing; the upgrade writes it into the triggers (patterns excepted) so nothing the user types changes.
#[test]
fn a_folder_prefix_becomes_part_of_its_triggers() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("snippets.db");
    Connection::open(&path)
        .expect("create")
        .execute_batch(
            "CREATE TABLE snippets (id INTEGER PRIMARY KEY, name TEXT NOT NULL, keyword TEXT NOT NULL DEFAULT '', body TEXT NOT NULL, uses INTEGER NOT NULL DEFAULT 0,
               folder TEXT NOT NULL DEFAULT '', mode TEXT NOT NULL DEFAULT 'auto', match_case INTEGER NOT NULL DEFAULT 0, enabled INTEGER NOT NULL DEFAULT 1,
               paste INTEGER NOT NULL DEFAULT 0, skip_apps TEXT NOT NULL DEFAULT '', folder_id INTEGER, triggers TEXT NOT NULL DEFAULT '',
               last_used INTEGER NOT NULL DEFAULT 0, insert_mode TEXT NOT NULL DEFAULT 'auto', ending TEXT NOT NULL DEFAULT '',
               inside_word INTEGER NOT NULL DEFAULT 0, propagate_case INTEGER NOT NULL DEFAULT 1, only_apps TEXT NOT NULL DEFAULT '');
             CREATE TABLE folders (id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE COLLATE NOCASE, enabled INTEGER NOT NULL DEFAULT 1,
               prefix TEXT NOT NULL DEFAULT '', only_apps TEXT NOT NULL DEFAULT '', skip_apps TEXT NOT NULL DEFAULT '');
             INSERT INTO folders (id, name, prefix) VALUES (1, 'Work', ';');
             INSERT INTO snippets (name, body, folder_id, triggers) VALUES ('Standup', 'text', 1, 'standup' || char(10) || 're:^x$');
             PRAGMA user_version = 2;",
        )
        .expect("fill");
    let conn = store::open(&path).expect("upgrade");
    assert_eq!(store::list(&conn).expect("list")[0].triggers, [";standup", "re:^x$"]);
    assert!(conn.prepare("SELECT prefix FROM folders").is_err(), "the prefix column is gone");
    assert_eq!(version(&conn), 3);
}

/// The expander and a page can open the old file at the same moment; both succeed and the data is upgraded once.
#[test]
fn two_first_opens_at_once_both_succeed_and_upgrade_once() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("snippets.db");
    first_version(&path);
    let start = std::sync::Barrier::new(2);
    std::thread::scope(|scope| {
        let open = || {
            start.wait();
            store::open(&path).map(|conn| store::list(&conn).map(|all| all.len()))
        };
        let (a, b) = (scope.spawn(open), scope.spawn(open));
        assert_eq!(a.join().expect("thread").expect("open").expect("list"), 2);
        assert_eq!(b.join().expect("thread").expect("open").expect("list"), 2);
    });
    let conn = store::open(&path).expect("reopen");
    assert_eq!((version(&conn), store::list(&conn).expect("list").len()), (3, 2));
}
