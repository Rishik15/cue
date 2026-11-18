//! File: notes/store.rs
//! Purpose: The notes database: a SQLite file in the app data folder, opened for one operation and closed again, so nothing is resident while idle.
//! Contents: Note, Entry — what the page gets; open — the file (created on first use, WAL so an edit survives a killed app); newest / create / get / save / neighbour — reading and writing the note on screen;
//! trash / restore / forget_empty / pin / promote — what the keys do to a note; list — the rows of the list view; sweep — old notes leave (no timer, called when the window opens); now, fail, note, front, first_line — helpers.
//! Every statement uses bound parameters. Notes are ordered by `sort`, newest first; editing a note does not move it, promoting does.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::Serialize;

/// Longest note kept, in characters: a paste of a whole log must not make every save slow.
pub const MAX_BODY: usize = 1_000_000;
/// Trash is emptied after this long.
const TRASH_DAYS: i64 = 30;
/// A day in milliseconds.
const DAY_MS: i64 = 86_400_000;
/// The most rows the list view returns. shortcut: no offset or cursor and no sign that rows were left out, so a library past 200 matching notes shows its newest 200; narrow the query to reach older ones, add paging when real libraries get there.
const LIST_LIMIT: i64 = 200;

/// A note as the page holds it.
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

/// A database error as the message the page shows. Input: the error. Returns the message.
fn fail(e: rusqlite::Error) -> String {
    format!("Notes: {e}")
}

/// The clock in milliseconds since the epoch (0 if the clock is before it). Returns the time.
pub fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

/// Opens (creating if missing) the notes database at `path` and prepares its table; returns the connection or a user-readable error. shortcut: one table, no migrations yet; the first change to it adds `user_version` steps like `snippets::store`.
pub fn open(path: &Path) -> Result<Connection, String> {
    let conn = Connection::open(path).map_err(|e| format!("Could not open the notes file: {e}"))?;
    conn.busy_timeout(std::time::Duration::from_secs(2)).map_err(|e| format!("Could not prepare the notes file: {e}"))?;
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         CREATE TABLE IF NOT EXISTS notes (id INTEGER PRIMARY KEY, body TEXT NOT NULL DEFAULT '', created INTEGER NOT NULL, updated INTEGER NOT NULL,
             sort INTEGER NOT NULL, pinned INTEGER NOT NULL DEFAULT 0, deleted INTEGER);
         CREATE INDEX IF NOT EXISTS notes_sort ON notes (sort);
         CREATE INDEX IF NOT EXISTS notes_live ON notes (pinned DESC, sort DESC, id DESC) WHERE deleted IS NULL;
         CREATE INDEX IF NOT EXISTS notes_trash ON notes (deleted DESC) WHERE deleted IS NOT NULL;",
    )
    .map_err(|e| format!("Could not prepare the notes file: {e}"))?;
    Ok(conn)
}

/// A row of `COLUMNS` as a note. Input: the row. Returns the note.
fn note(r: &Row) -> rusqlite::Result<Note> {
    Ok(Note { id: r.get(0)?, body: r.get(1)?, pinned: r.get::<_, i64>(2)? != 0 })
}

/// The columns a note is read from, in the order `note` takes them.
const COLUMNS: &str = "id, body, pinned";

/// The next place at the front of the order, as a SQL expression over the clock parameter `?1`-style `now`: the later of the clock and one past the highest place, so notes made, restored or promoted back to back never share a place and a clock that ran backwards cannot put one behind older notes. One statement, so SQLite's single writer allocates it atomically.
fn front(now: &str) -> String {
    format!("max({now}, COALESCE((SELECT MAX(sort) FROM notes), 0) + 1)")
}

/// The newest note, or a new empty one when there is none.
pub fn newest(conn: &Connection) -> Result<Note, String> {
    let found = conn
        .query_row(&format!("SELECT {COLUMNS} FROM notes WHERE deleted IS NULL ORDER BY sort DESC, id DESC LIMIT 1"), [], note)
        .optional()
        .map_err(fail)?;
    found.map_or_else(|| create(conn), Ok)
}

/// A new empty note at the front of the order. Input: the connection. Returns the note.
pub fn create(conn: &Connection) -> Result<Note, String> {
    let now = now();
    conn.execute(&format!("INSERT INTO notes (body, created, updated, sort) VALUES ('', ?1, ?1, {})", front("?1")), [now]).map_err(fail)?;
    Ok(Note { id: conn.last_insert_rowid(), body: String::new(), pinned: false })
}

/// The live note `id`. Inputs: the connection, the id. Returns the note, or None when it is gone or in the trash.
pub fn get(conn: &Connection, id: i64) -> Result<Option<Note>, String> {
    conn.query_row(&format!("SELECT {COLUMNS} FROM notes WHERE id = ?1 AND deleted IS NULL"), [id], note).optional().map_err(fail)
}

/// Stores a note's text. Inputs: the connection, the id, the text. Returns an error when the text is too long or the note is gone.
pub fn save(conn: &Connection, id: i64, body: &str) -> Result<(), String> {
    if body.chars().count() > MAX_BODY {
        return Err("The note is too long to save.".into());
    }
    match conn.execute("UPDATE notes SET body = ?2, updated = ?3 WHERE id = ?1", params![id, body, now()]).map_err(fail)? {
        0 => Err("The note is gone.".into()),
        _ => Ok(()),
    }
}

/// The note next to `id` in the order: the newer one (`newer`) or the older one. None at either end.
pub fn neighbour(conn: &Connection, id: i64, newer: bool) -> Result<Option<Note>, String> {
    let (cmp, order) = if newer { (">", "ASC") } else { ("<", "DESC") };
    let sql = format!(
        "SELECT {COLUMNS} FROM notes WHERE deleted IS NULL AND (sort, id) {cmp} (SELECT sort, id FROM notes WHERE id = ?1) ORDER BY sort {order}, id {order} LIMIT 1"
    );
    conn.query_row(&sql, [id], note).optional().map_err(fail)
}

/// Moves a note to the trash. Inputs: the connection, the id.
pub fn trash(conn: &Connection, id: i64) -> Result<(), String> {
    conn.execute("UPDATE notes SET deleted = ?2 WHERE id = ?1", params![id, now()]).map(drop).map_err(fail)
}

/// Brings a note back from the trash to the front. Inputs: the connection, the id.
pub fn restore(conn: &Connection, id: i64) -> Result<(), String> {
    conn.execute(&format!("UPDATE notes SET deleted = NULL, sort = {} WHERE id = ?1", front("?2")), params![id, now()])
        .map(drop)
        .map_err(fail)
}

/// A note left empty is not worth keeping: it goes for good (not to the trash). Inputs: the connection, the id.
pub fn forget_empty(conn: &Connection, id: i64) -> Result<(), String> {
    conn.execute("DELETE FROM notes WHERE id = ?1 AND body = ''", [id]).map(drop).map_err(fail)
}

/// Pins or unpins a note. Inputs: the connection, the id, the new state.
pub fn pin(conn: &Connection, id: i64, pinned: bool) -> Result<(), String> {
    conn.execute("UPDATE notes SET pinned = ?2 WHERE id = ?1", params![id, pinned]).map(drop).map_err(fail)
}

/// Makes a note the newest. Inputs: the connection, the id.
pub fn promote(conn: &Connection, id: i64) -> Result<(), String> {
    conn.execute(&format!("UPDATE notes SET sort = {} WHERE id = ?1", front("?2")), params![id, now()]).map(drop).map_err(fail)
}

/// The rows of the list view (live notes, or the trash), pinned first. `query` matches anywhere in the text, case-insensitively for ASCII.
pub fn list(conn: &Connection, query: &str, in_trash: bool) -> Result<Vec<Entry>, String> {
    let escaped = query.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
    let (clause, order) =
        if in_trash { ("deleted IS NOT NULL", "deleted DESC") } else { ("deleted IS NULL", "pinned DESC, sort DESC, id DESC") };
    let sql = format!("SELECT id, substr(body, 1, 200), updated, pinned, length(replace(body, char(10), '')) FROM notes WHERE {clause} AND body LIKE ?1 ESCAPE '\\' ORDER BY {order} LIMIT {LIST_LIMIT}");
    let mut stmt = conn.prepare(&sql).map_err(fail)?;
    let rows = stmt
        .query_map([format!("%{escaped}%")], |r| {
            let head: String = r.get(1)?;
            Ok(Entry { id: r.get(0)?, first: first_line(&head), updated: r.get(2)?, pinned: r.get::<_, i64>(3)? != 0, chars: r.get(4)? })
        })
        .map_err(fail)?;
    rows.collect::<rusqlite::Result<_>>().map_err(fail)
}

/// The first line with text, trimmed and cut to 120 characters. Input: the start of a note. Returns the line.
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
