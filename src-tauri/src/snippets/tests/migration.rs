//! File: snippets/tests/migration.rs
//! Purpose: Protect the one schema of the snippets file: a new file gets it, a 0.3 file (version 3) and a version-4 file upgrade in one step keeping every snippet byte for byte, older or newer files are refused untouched, a failed or refused upgrade changes nothing, two first opens at once do not both upgrade, and the bench's seeded library stays at the schema.
//! Contents: tests that build files by hand and open them with `store::open`; the guard on `bench/snippets.db`.

use std::path::Path;

use rusqlite::Connection;

use crate::snippets::model::Insert;
use crate::snippets::store;

const OLDER: &str = "This snippets file is from a Cue older than 0.2. Install Cue 0.3 first, run it once, then update again.";
const NEWER: &str = "The snippets file was written by a newer Cue. Update Cue to use it.";

/// The version-3 tables (Cue 0.2 and 0.3), dead columns included. Input: `name` column type, so a test can allow a NULL the final table refuses.
fn version_3_tables(name_type: &str) -> String {
    format!(
        "CREATE TABLE snippets (id INTEGER PRIMARY KEY, name {name_type}, keyword TEXT NOT NULL DEFAULT '', body TEXT NOT NULL, uses INTEGER NOT NULL DEFAULT 0,
           folder TEXT NOT NULL DEFAULT '', mode TEXT NOT NULL DEFAULT 'auto', match_case INTEGER NOT NULL DEFAULT 0, enabled INTEGER NOT NULL DEFAULT 1,
           paste INTEGER NOT NULL DEFAULT 0, skip_apps TEXT NOT NULL DEFAULT '', folder_id INTEGER, triggers TEXT NOT NULL DEFAULT '',
           last_used INTEGER NOT NULL DEFAULT 0, insert_mode TEXT NOT NULL DEFAULT 'auto', ending TEXT NOT NULL DEFAULT '',
           inside_word INTEGER NOT NULL DEFAULT 0, propagate_case INTEGER NOT NULL DEFAULT 1, only_apps TEXT NOT NULL DEFAULT '');
         CREATE TABLE folders (id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE COLLATE NOCASE, enabled INTEGER NOT NULL DEFAULT 1,
           only_apps TEXT NOT NULL DEFAULT '', skip_apps TEXT NOT NULL DEFAULT '');
         PRAGMA user_version = 3;"
    )
}

/// A version-3 file with three snippets (one per stored insert value), a folder and usage counts. Input: where to write it.
fn version_3_file(path: &Path) {
    Connection::open(path)
        .expect("create")
        .execute_batch(&format!(
            "{}
             INSERT INTO folders (id, name, enabled, only_apps) VALUES (7, 'Work', 0, 'Slack');
             INSERT INTO snippets (id, name, keyword, body, uses, last_used, folder_id, triggers, insert_mode, mode, ending, skip_apps)
               VALUES (1, 'Sig', ':old', 'Best', 5, 1700000000, 7, ':sig' || char(10) || ';sig', 'rich', 'immediate', '.,', 'Code'),
                      (2, 'Mail', '', 'a@b.com', 2, 0, NULL, ';em', 'auto', 'auto', '', ''),
                      (3, 'Off', '', 'x', 0, 0, NULL, 'off', 'paste', 'delimiter', '', '');
             UPDATE snippets SET enabled = 0 WHERE id = 3;",
            version_3_tables("TEXT NOT NULL")
        ))
        .expect("fill");
}

/// The version-4 tables (Cue 0.4 until the integrity schema): no relation, no domain checks, reusable ids.
const VERSION_4_TABLES: &str = "CREATE TABLE snippets (id INTEGER PRIMARY KEY, name TEXT NOT NULL, triggers TEXT NOT NULL DEFAULT '', body TEXT NOT NULL,
      uses INTEGER NOT NULL DEFAULT 0, last_used INTEGER NOT NULL DEFAULT 0, folder_id INTEGER, enabled INTEGER NOT NULL DEFAULT 1,
      \"insert\" TEXT NOT NULL DEFAULT 'paste' CHECK (\"insert\" IN ('paste', 'rich')), only_apps TEXT NOT NULL DEFAULT '', skip_apps TEXT NOT NULL DEFAULT '');
    CREATE TABLE folders (id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE COLLATE NOCASE, enabled INTEGER NOT NULL DEFAULT 1,
      only_apps TEXT NOT NULL DEFAULT '', skip_apps TEXT NOT NULL DEFAULT '');
    PRAGMA user_version = 4;";

/// A version-4 file with a folder, two snippets (ids 5 and 9, a multi-line trigger list, a large non-ASCII text, use counts) and `extra` SQL run after them. Input: where to write it and the extra SQL.
fn version_4_file(path: &Path, extra: &str) {
    Connection::open(path)
        .expect("create")
        .execute_batch(&format!(
            "{VERSION_4_TABLES}
             INSERT INTO folders (id, name, enabled, only_apps) VALUES (3, 'Work', 0, 'Slack' || char(9) || 'slack');
             INSERT INTO snippets (id, name, triggers, body, uses, last_used, folder_id, enabled, \"insert\", skip_apps)
               VALUES (5, 'Sig', ':sig' || char(10) || ';sig', 'Best', 9, 1700000000, 3, 1, 'rich', 'Code' || char(9) || 'code'),
                      (9, 'Big', ':big', replace(hex(zeroblob(15000)), '00', 'é'), 0, 0, NULL, 0, 'paste', '');
             {extra}"
        ))
        .expect("fill");
}

/// Every row of `table` as SQLite stores it, by id. Inputs: the connection and the table. Returns the rows, each value as stored.
fn raw_rows(conn: &Connection, table: &str) -> Vec<Vec<rusqlite::types::Value>> {
    let mut stmt = conn.prepare(&format!("SELECT * FROM {table} ORDER BY id")).expect("select");
    let width = stmt.column_count();
    stmt.query_map([], |r| (0..width).map(|i| r.get(i)).collect()).expect("rows").collect::<Result<_, _>>().expect("read")
}

/// A version-4 file upgrades in one step: every snippet and folder keeps its id and every value byte for byte, ids carry on above the highest (never reused), and the schema then enforces folders and domains.
#[test]
fn a_version_4_file_upgrades_keeping_every_row() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("snippets.db");
    version_4_file(&path, "");
    let before = Connection::open(&path).expect("open");
    let (snippets, groups) = (raw_rows(&before, "snippets"), raw_rows(&before, "folders"));
    drop(before);
    let conn = store::open(&path).expect("upgrade");
    assert_eq!(version(&conn), 5);
    assert_eq!((raw_rows(&conn, "snippets"), raw_rows(&conn, "folders")), (snippets, groups));
    assert_eq!(store::get(&conn, 9).expect("get").expect("big").body, "é".repeat(15000));
    conn.execute("INSERT INTO snippets (name, body) VALUES ('New', 'x')", []).expect("insert");
    assert_eq!(conn.last_insert_rowid(), 10);
    assert!(conn.execute("INSERT INTO snippets (name, body, folder_id) VALUES ('a', 'b', 77)", []).is_err(), "folders enforced");
    assert!(conn.execute("INSERT INTO snippets (name, body, uses) VALUES ('a', 'b', -1)", []).is_err(), "domains enforced");
}

/// A snippet whose folder was deleted without unfiling it (an orphan id, which always read as no folder) is upgraded unfiled, so a later folder can never adopt it.
#[test]
fn an_orphan_folder_id_is_upgraded_as_unfiled() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("snippets.db");
    version_4_file(&path, "UPDATE snippets SET folder_id = 4 WHERE id = 9;");
    let conn = store::open(&path).expect("upgrade");
    assert_eq!(
        (store::get(&conn, 9).expect("get").expect("big").folder_id, store::get(&conn, 5).expect("get").expect("sig").folder_id),
        (None, Some(3))
    );
}

/// A version-4 upgrade that fails half-way (a table already named `folders_old` stops the second rename after the first) or meets a row outside its domain (a negative use count, written by hand) rolls back whole: the file stays byte for byte at version 4.
#[test]
fn a_failed_version_4_upgrade_leaves_the_file_as_it_was() {
    let dir = tempfile::tempdir().expect("dir");
    for (file, extra) in [("way.db", "CREATE TABLE folders_old (x);"), ("damaged.db", "UPDATE snippets SET uses = -1 WHERE id = 5;")] {
        let path = dir.path().join(file);
        version_4_file(&path, extra);
        let before = std::fs::read(&path).expect("read");
        assert!(store::open(&path).is_err(), "{file} upgraded");
        assert_eq!(std::fs::read(&path).expect("read"), before, "{file} is untouched");
        assert_eq!(version(&Connection::open(&path).expect("reopen")), 4, "{file}");
    }
}

/// The user version of an open database. Input: the connection. Returns it.
fn version(conn: &Connection) -> i64 {
    conn.query_row("PRAGMA user_version", [], |r| r.get(0)).expect("version")
}

/// A new file gets the final schema at version 5: no dead column, and the insert column takes only paste or rich.
#[test]
fn a_new_file_gets_the_final_schema() {
    let dir = tempfile::tempdir().expect("dir");
    let conn = store::open(&dir.path().join("snippets.db")).expect("create");
    assert_eq!(version(&conn), 5);
    assert!(conn.prepare("SELECT keyword FROM snippets").is_err(), "keyword is gone");
    assert!(conn.prepare("SELECT mode FROM snippets").is_err(), "mode is gone");
    assert!(conn.execute("INSERT INTO snippets (name, body, \"insert\") VALUES ('a', 'b', 'auto')", []).is_err(), "auto is refused");
    assert!(conn.execute("INSERT INTO snippets (name, body, \"insert\") VALUES ('a', 'b', 'rich')", []).is_ok());
}

/// A 0.3 file upgrades in one step: every snippet, trigger, use count, folder and program list stays; rich stays rich, anything else becomes paste.
#[test]
fn a_version_3_file_upgrades_in_one_step() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("snippets.db");
    version_3_file(&path);
    let conn = store::open(&path).expect("upgrade");
    assert_eq!(version(&conn), 5);
    let all = store::list(&conn).expect("list");
    let row = |s: &crate::snippets::model::Snippet| {
        (s.id, s.name.clone(), s.triggers.clone(), s.uses, s.last_used, s.folder_id, s.enabled, s.insert)
    };
    assert_eq!(
        all.iter().map(row).collect::<Vec<_>>(),
        [
            (1, "Sig".into(), vec![":sig".to_string(), ";sig".to_string()], 5, 1_700_000_000, Some(7), true, Insert::Rich),
            (2, "Mail".into(), vec![";em".to_string()], 2, 0, None, true, Insert::Paste),
            (3, "Off".into(), vec!["off".to_string()], 0, 0, None, false, Insert::Paste),
        ]
    );
    assert_eq!((all[0].body.as_str(), all[0].skip_apps.as_slice()), ("Best", ["Code".to_string()].as_slice()));
    let work = &crate::snippets::folders::folders(&conn).expect("folders")[0];
    assert_eq!(
        (work.id, work.name.as_str(), work.enabled, work.only_apps.as_slice()),
        (7, "Work", false, ["Slack".to_string()].as_slice())
    );
    assert!(conn.prepare("SELECT insert_mode FROM snippets").is_err(), "dead columns are gone");
    drop(conn);
    assert_eq!(store::list(&store::open(&path).expect("reopen")).expect("list").len(), 3);
}

/// A file from before 0.2 (with a snippets table) is refused with a plain message and left byte for byte; a newer file is refused too.
#[test]
fn an_older_file_is_refused_untouched() {
    let dir = tempfile::tempdir().expect("dir");
    for (old, stamp) in [("v0.db", 0), ("v2.db", 2)] {
        let path = dir.path().join(old);
        Connection::open(&path)
            .expect("create")
            .execute_batch(&format!(
                "CREATE TABLE snippets (id INTEGER PRIMARY KEY, name TEXT NOT NULL, keyword TEXT NOT NULL DEFAULT '', body TEXT NOT NULL, uses INTEGER NOT NULL DEFAULT 0);
                 INSERT INTO snippets (name, keyword, body) VALUES ('Sig', ':sig', 'Best');
                 PRAGMA user_version = {stamp};"
            ))
            .expect("fill");
        let before = std::fs::read(&path).expect("read");
        assert_eq!(store::open(&path).err().map(String::from).as_deref(), Some(OLDER), "{old}");
        assert_eq!(std::fs::read(&path).expect("read"), before, "{old} is untouched");
    }
    let newer = dir.path().join("v6.db");
    Connection::open(&newer)
        .expect("create")
        .execute_batch(&format!("{} PRAGMA user_version = 6;", version_3_tables("TEXT NOT NULL")))
        .expect("fill");
    let before = std::fs::read(&newer).expect("read");
    assert_eq!(store::open(&newer).err().map(String::from).as_deref(), Some(NEWER));
    assert_eq!(std::fs::read(&newer).expect("read"), before);
}

/// An upgrade that cannot copy a row (a NULL name the final table refuses) rolls back whole: the file stays at version 3 with its old table.
#[test]
fn a_failed_upgrade_leaves_the_file_as_it_was() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("snippets.db");
    Connection::open(&path)
        .expect("create")
        .execute_batch(&format!("{} INSERT INTO snippets (name, body, triggers) VALUES (NULL, 'b', 'x');", version_3_tables("TEXT")))
        .expect("fill");
    assert!(store::open(&path).is_err());
    let conn = Connection::open(&path).expect("reopen");
    assert_eq!(version(&conn), 3);
    assert_eq!(conn.query_row("SELECT COUNT(insert_mode) FROM snippets", [], |r| r.get::<_, i64>(0)).expect("old table"), 1);
}

/// The expander and a page can open a 0.3 file at the same moment; both succeed and the file is upgraded once.
#[test]
fn two_first_opens_at_once_both_succeed_and_upgrade_once() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("snippets.db");
    version_3_file(&path);
    let start = std::sync::Barrier::new(2);
    std::thread::scope(|scope| {
        let open = || {
            start.wait();
            store::open(&path).map(|conn| store::list(&conn).map(|all| all.len()))
        };
        let (a, b) = (scope.spawn(open), scope.spawn(open));
        assert_eq!(a.join().expect("thread").expect("open").expect("list"), 3);
        assert_eq!(b.join().expect("thread").expect("open").expect("list"), 3);
    });
    let conn = store::open(&path).expect("reopen");
    assert_eq!((version(&conn), store::list(&conn).expect("list").len()), (5, 3));
}

/// The bench's seeded library (`bench/snippets.db`) is at the current schema, so the expansion stage measures the file Cue really reads: its tables and version equal a new file's, and it opens without an upgrade. When the schema moves on, rebuild the fixture with the same five snippets.
#[test]
fn the_bench_fixture_is_at_the_current_schema() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../bench/snippets.db");
    let dir = tempfile::tempdir().expect("temp dir");
    let (copy, fresh) = (dir.path().join("fixture.db"), dir.path().join("fresh.db"));
    std::fs::copy(&fixture, &copy).expect("copy the fixture");
    let schema = |path: &Path| {
        let conn = Connection::open(path).expect("open");
        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).expect("version");
        let mut stmt = conn.prepare("SELECT type, name, sql FROM sqlite_master ORDER BY name").expect("schema");
        let rows: Vec<(String, String, Option<String>)> =
            stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).expect("rows").collect::<Result<_, _>>().expect("read");
        (version, rows)
    };
    drop(store::open(&fresh).expect("a new file"));
    assert_eq!(schema(&copy), schema(&fresh));
    let snippets = store::list(&store::open(&copy).expect("the fixture opens")).expect("list");
    let bodies: Vec<&str> = snippets.iter().map(|s| s.body.as_str()).collect();
    assert_eq!(snippets.len(), 5);
    assert!(bodies.iter().any(|b| b.contains("{snippet:")) && bodies.iter().any(|b| b.contains("{counter}")));
    assert!(bodies.iter().any(|b| b.len() > 4096), "one text is too big to keep in memory");
}
