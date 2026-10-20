//! Purpose: The notes database: a SQLite file in the app data folder, opened for one operation and closed again, so nothing is resident while idle.
//! Contents: open — the file (created on first use, WAL so an edit survives a killed app); newest / create / get / save / neighbour — reading and writing the note on screen;
//! trash / restore / forget_empty / pin / promote — what the keys do to a note; list — the rows of the list view; sweep — old notes leave (no timer, called when the window opens).
//! Every statement uses bound parameters. Notes are ordered by `sort`, newest first; editing a note does not move it, promoting does.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::Serialize;

/// Longest note kept, in characters: a paste of a whole log must not make every save slow.
pub const MAX_BODY: usize = 1_000_000;
/// Trash is emptied after this long.
const TRASH_DAYS: i64 = 30;
const DAY_MS: i64 = 86_400_000;
const LIST_LIMIT: i64 = 200;

#[derive(Serialize, Clone)]
pub struct Note {
    pub id: i64,
    pub body: String,
    pub pinned: bool,
}

/// One line of the list view.
#[derive(Serialize)]
pub struct Entry {
    id: i64,
    first: String,
    updated: i64,
    pinned: bool,
    chars: i64,
}

fn fail(e: rusqlite::Error) -> String {
    format!("Notes: {e}")
}

pub fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

// ponytail: one table, no migrations yet; the first change to it adds `user_version` steps like `snippets::store`.
pub fn open(path: &Path) -> Result<Connection, String> {
    let conn = Connection::open(path).map_err(|e| format!("Could not open the notes file: {e}"))?;
    // A save that meets another write waits for it instead of failing: the page does not retry a save.
    conn.busy_timeout(std::time::Duration::from_secs(2)).map_err(|e| format!("Could not prepare the notes file: {e}"))?;
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         CREATE TABLE IF NOT EXISTS notes (id INTEGER PRIMARY KEY, body TEXT NOT NULL DEFAULT '', created INTEGER NOT NULL, updated INTEGER NOT NULL,
             sort INTEGER NOT NULL, pinned INTEGER NOT NULL DEFAULT 0, deleted INTEGER);",
    )
    .map_err(|e| format!("Could not prepare the notes file: {e}"))?;
    Ok(conn)
}

fn note(r: &Row) -> rusqlite::Result<Note> {
    Ok(Note { id: r.get(0)?, body: r.get(1)?, pinned: r.get::<_, i64>(2)? != 0 })
}

const COLUMNS: &str = "id, body, pinned";

/// The newest note, or a new empty one when there is none.
pub fn newest(conn: &Connection) -> Result<Note, String> {
    let found = conn
        .query_row(&format!("SELECT {COLUMNS} FROM notes WHERE deleted IS NULL ORDER BY sort DESC LIMIT 1"), [], note)
        .optional()
        .map_err(fail)?;
    found.map_or_else(|| create(conn), Ok)
}

pub fn create(conn: &Connection) -> Result<Note, String> {
    let now = now();
    conn.execute("INSERT INTO notes (body, created, updated, sort) VALUES ('', ?1, ?1, ?1)", [now]).map_err(fail)?;
    Ok(Note { id: conn.last_insert_rowid(), body: String::new(), pinned: false })
}

pub fn get(conn: &Connection, id: i64) -> Result<Option<Note>, String> {
    conn.query_row(&format!("SELECT {COLUMNS} FROM notes WHERE id = ?1 AND deleted IS NULL"), [id], note).optional().map_err(fail)
}

pub fn save(conn: &Connection, id: i64, body: &str) -> Result<(), String> {
    if body.chars().count() > MAX_BODY {
        return Err("The note is too long to save.".into());
    }
    conn.execute("UPDATE notes SET body = ?2, updated = ?3 WHERE id = ?1", params![id, body, now()]).map(drop).map_err(fail)
}

/// The note next to `id` in the order: the newer one (`newer`) or the older one. None at either end.
pub fn neighbour(conn: &Connection, id: i64, newer: bool) -> Result<Option<Note>, String> {
    let (cmp, order) = if newer { (">", "ASC") } else { ("<", "DESC") };
    let sql = format!(
        "SELECT {COLUMNS} FROM notes WHERE deleted IS NULL AND sort {cmp} (SELECT sort FROM notes WHERE id = ?1) ORDER BY sort {order} LIMIT 1"
    );
    conn.query_row(&sql, [id], note).optional().map_err(fail)
}

pub fn trash(conn: &Connection, id: i64) -> Result<(), String> {
    conn.execute("UPDATE notes SET deleted = ?2 WHERE id = ?1", params![id, now()]).map(drop).map_err(fail)
}

pub fn restore(conn: &Connection, id: i64) -> Result<(), String> {
    conn.execute("UPDATE notes SET deleted = NULL, sort = ?2 WHERE id = ?1", params![id, now()]).map(drop).map_err(fail)
}

/// A note left empty is not worth keeping: it goes for good (not to the trash).
pub fn forget_empty(conn: &Connection, id: i64) -> Result<(), String> {
    conn.execute("DELETE FROM notes WHERE id = ?1 AND body = ''", [id]).map(drop).map_err(fail)
}

pub fn pin(conn: &Connection, id: i64, pinned: bool) -> Result<(), String> {
    conn.execute("UPDATE notes SET pinned = ?2 WHERE id = ?1", params![id, pinned]).map(drop).map_err(fail)
}

pub fn promote(conn: &Connection, id: i64) -> Result<(), String> {
    conn.execute("UPDATE notes SET sort = ?2 WHERE id = ?1", params![id, now()]).map(drop).map_err(fail)
}

/// The rows of the list view (live notes, or the trash), pinned first. `query` matches anywhere in the text, case-insensitively for ASCII.
pub fn list(conn: &Connection, query: &str, in_trash: bool) -> Result<Vec<Entry>, String> {
    let escaped = query.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
    let (clause, order) = if in_trash { ("deleted IS NOT NULL", "deleted DESC") } else { ("deleted IS NULL", "pinned DESC, sort DESC") };
    let sql = format!("SELECT id, substr(body, 1, 200), updated, pinned, length(body) FROM notes WHERE {clause} AND body LIKE ?1 ESCAPE '\\' ORDER BY {order} LIMIT {LIST_LIMIT}");
    let mut stmt = conn.prepare(&sql).map_err(fail)?;
    let rows = stmt
        .query_map([format!("%{escaped}%")], |r| {
            let head: String = r.get(1)?;
            Ok(Entry { id: r.get(0)?, first: first_line(&head), updated: r.get(2)?, pinned: r.get::<_, i64>(3)? != 0, chars: r.get(4)? })
        })
        .map_err(fail)?;
    rows.collect::<rusqlite::Result<_>>().map_err(fail)
}

fn first_line(head: &str) -> String {
    head.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("").chars().take(120).collect()
}

/// Old notes leave: unpinned ones untouched for `expire_days` go to the trash (0 keeps them), and the trash is emptied after thirty days.
pub fn sweep(conn: &Connection, expire_days: i64) -> Result<(), String> {
    let now = now();
    if expire_days > 0 {
        conn.execute(
            "UPDATE notes SET deleted = ?1 WHERE deleted IS NULL AND pinned = 0 AND updated < ?2",
            params![now, now - expire_days * DAY_MS],
        )
        .map_err(fail)?;
    }
    conn.execute("DELETE FROM notes WHERE deleted IS NOT NULL AND deleted < ?1", [now - TRASH_DAYS * DAY_MS]).map(drop).map_err(fail)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE notes (id INTEGER PRIMARY KEY, body TEXT NOT NULL DEFAULT '', created INTEGER NOT NULL, updated INTEGER NOT NULL,
             sort INTEGER NOT NULL, pinned INTEGER NOT NULL DEFAULT 0, deleted INTEGER);",
        )
        .unwrap();
        conn
    }

    fn add(conn: &Connection, body: &str, sort: i64) -> i64 {
        conn.execute("INSERT INTO notes (body, created, updated, sort) VALUES (?1, ?2, ?2, ?2)", params![body, sort]).unwrap();
        conn.last_insert_rowid()
    }

    #[test]
    fn order_navigation_and_trash() {
        let conn = memory();
        let (a, b, c) = (add(&conn, "a", 1), add(&conn, "b", 2), add(&conn, "c", 3));
        assert_eq!(newest(&conn).unwrap().id, c);
        assert_eq!(neighbour(&conn, c, false).unwrap().unwrap().id, b);
        assert_eq!(neighbour(&conn, a, false).unwrap().map(|n| n.id), None);
        assert_eq!(neighbour(&conn, c, true).unwrap().map(|n| n.id), None);
        trash(&conn, c).unwrap();
        assert_eq!(newest(&conn).unwrap().id, b);
        assert_eq!(list(&conn, "", true).unwrap().len(), 1);
        restore(&conn, c).unwrap();
        assert_eq!(newest(&conn).unwrap().id, c);
    }

    #[test]
    fn empty_notes_pinning_search_and_sweep() {
        let conn = memory();
        let (e, a) = (add(&conn, "", 1), add(&conn, "100% sure\nsecond", 2));
        forget_empty(&conn, e).unwrap();
        forget_empty(&conn, a).unwrap(); // not empty: stays
        assert_eq!(list(&conn, "", false).unwrap().len(), 1);
        assert_eq!(list(&conn, "100%", false).unwrap()[0].first, "100% sure");
        assert!(list(&conn, "%", false).unwrap().len() == 1 && list(&conn, "zzz", false).unwrap().is_empty());
        let old = add(&conn, "old", 0);
        conn.execute("UPDATE notes SET updated = 0 WHERE id = ?1", [old]).unwrap();
        pin(&conn, a, true).unwrap();
        conn.execute("UPDATE notes SET updated = 0", []).unwrap();
        sweep(&conn, 7).unwrap();
        assert_eq!(list(&conn, "", false).unwrap().len(), 1); // the pinned one stays
        sweep(&conn, 0).unwrap();
        assert_eq!(list(&conn, "", true).unwrap().len(), 1); // the other is in the trash, not gone
        conn.execute("UPDATE notes SET deleted = 1", []).unwrap();
        sweep(&conn, 0).unwrap();
        assert!(list(&conn, "", true).unwrap().is_empty());
    }
}
