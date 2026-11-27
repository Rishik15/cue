//! File: snippets/store.rs
//! Purpose: The snippet database: a SQLite file in the app data folder, opened for one operation and closed again, so nothing is resident while idle.
//! Contents: Failure — a failed store operation, SQLite's error kept typed; open / open_waiting — the file (created, or brought to the one schema by `setup`, on first use); in_write — the one write transaction every change runs in; snapshot — the one read transaction a whole read runs in; check_library — whether the whole library's triggers can coexist; list / list_for_index / get / effective_all — reads (the folders are in `folders`); save / remove / bump — snippet writes (many at once are in `bulk`).
//! Every statement uses bound parameters; text is never formatted into SQL. The file has one schema (version 5): ids are never reused, a snippet's folder is a real folder or none (enforced on every connection), and every row keeps to its domain. A 0.3 file (version 3) or a version-4 file is rebuilt to it once, and an older or newer file is refused untouched.

use std::fmt;
use std::path::Path;
use std::time::Duration;

use rusqlite::types::Type;
use rusqlite::{named_params, params, Connection, ErrorCode, Row};

use super::check::{check, clean, TriggerSet, MAX_SNIPPETS};
use super::folders::folders;
use super::model::{parse_name, Draft, Folder, Snippet};
use super::policy::{Effective, Policy};

/// The one schema of the file; `setup` creates it or rebuilds a version-3 or version-4 file to it.
const LATEST: i64 = 5;
/// The tables of version 5: AUTOINCREMENT ids (a deleted id is never handed out again, so a stale edit, delete or use count cannot land on a newcomer), a folder reference that a folder deletion clears, and CHECKs for every column's domain.
const TABLES: &str = r#"CREATE TABLE folders (id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE COLLATE NOCASE CHECK (typeof(name) = 'text' AND trim(name) <> ''),
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    only_apps TEXT NOT NULL DEFAULT '' CHECK (typeof(only_apps) = 'text'),
    skip_apps TEXT NOT NULL DEFAULT '' CHECK (typeof(skip_apps) = 'text'));
CREATE TABLE snippets (id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL CHECK (typeof(name) = 'text' AND trim(name) <> ''),
    triggers TEXT NOT NULL DEFAULT '' CHECK (typeof(triggers) = 'text'),
    body TEXT NOT NULL CHECK (typeof(body) = 'text' AND trim(body) <> ''),
    uses INTEGER NOT NULL DEFAULT 0 CHECK (typeof(uses) = 'integer' AND uses >= 0),
    last_used INTEGER NOT NULL DEFAULT 0 CHECK (typeof(last_used) = 'integer' AND last_used >= 0),
    folder_id INTEGER REFERENCES folders (id) ON DELETE SET NULL,
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    "insert" TEXT NOT NULL DEFAULT 'paste' CHECK ("insert" IN ('paste', 'rich')),
    only_apps TEXT NOT NULL DEFAULT '' CHECK (typeof(only_apps) = 'text'),
    skip_apps TEXT NOT NULL DEFAULT '' CHECK (typeof(skip_apps) = 'text'));"#;
/// The first half of an upgrade: the old tables moved aside (a version-3 file without folders gets the empty old table first), so `TABLES` creates the new ones under their own names.
const SET_ASIDE: &str = r#"CREATE TABLE IF NOT EXISTS folders (id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE COLLATE NOCASE,
    enabled INTEGER NOT NULL DEFAULT 1, only_apps TEXT NOT NULL DEFAULT '', skip_apps TEXT NOT NULL DEFAULT '');
ALTER TABLE snippets RENAME TO snippets_old;
ALTER TABLE folders RENAME TO folders_old;"#;
/// The second half: every row copied with its id, uses and text; `{insert}` is the SQL that reads the old row's insert mode. A folder id that names no folder (left by a deleted folder) becomes none, as it already read.
const COPY: &str = r#"INSERT INTO folders (id, name, enabled, only_apps, skip_apps) SELECT id, name, enabled, only_apps, skip_apps FROM folders_old;
INSERT INTO snippets (id, name, triggers, body, uses, last_used, folder_id, enabled, "insert", only_apps, skip_apps)
    SELECT id, name, triggers, body, uses, last_used, (SELECT f.id FROM folders f WHERE f.id = snippets_old.folder_id), enabled, {insert},
        only_apps, skip_apps FROM snippets_old;
DROP TABLE snippets_old;
DROP TABLE folders_old;"#;
/// How long a statement waits for another connection's lock before it fails with SQLITE_BUSY.
const WAIT: Duration = Duration::from_secs(5);
const OLDER: &str = "This snippets file is from a Cue older than 0.2. Install Cue 0.3 first, run it once, then update again.";
const NEWER: &str = "The snippets file was written by a newer Cue. Update Cue to use it.";

/// A failed store operation: SQLite's own error, kept typed so a caller can tell a passing lock from damage, or a refusal already worded for the user.
#[derive(Debug)]
pub enum Failure {
    Sqlite(rusqlite::Error),
    Refused(String),
}

impl Failure {
    /// Whether another connection held the file past the wait (SQLITE_BUSY): the one failure that may pass by itself. A lock within this connection (SQLITE_LOCKED), damage, a refused version or a policy refusal is not. Returns the answer.
    pub fn is_busy(&self) -> bool {
        matches!(self, Failure::Sqlite(e) if e.sqlite_error_code() == Some(ErrorCode::DatabaseBusy))
    }
}

impl fmt::Display for Failure {
    /// The message the page shows.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Failure::Sqlite(e) => write!(f, "Snippets: {e}"),
            Failure::Refused(message) => f.write_str(message),
        }
    }
}

impl From<rusqlite::Error> for Failure {
    /// A SQLite error, kept as it is.
    fn from(e: rusqlite::Error) -> Self {
        Failure::Sqlite(e)
    }
}

impl From<String> for Failure {
    /// A refusal worded for the user.
    fn from(message: String) -> Self {
        Failure::Refused(message)
    }
}

impl From<Failure> for String {
    /// The failure as the page's message, once no caller needs to tell kinds apart.
    fn from(f: Failure) -> Self {
        f.to_string()
    }
}

/// Opens (and creates or upgrades) the snippets file. Input: its path. Returns the connection; a second writer waits up to 5 seconds for the lock instead of failing.
pub fn open(path: &Path) -> Result<Connection, Failure> {
    open_waiting(path, WAIT)
}

/// `open` with `wait` as the longest one statement waits for another connection's lock. Inputs: the path and the wait. Returns the connection, with foreign keys enforced.
pub fn open_waiting(path: &Path, wait: Duration) -> Result<Connection, Failure> {
    let conn = Connection::open(path)?;
    conn.busy_timeout(wait)?;
    conn.pragma_update(None, "foreign_keys", true)?;
    setup(&conn)?;
    Ok(conn)
}

/// Brings the file to the one schema: a file at it is only read; otherwise, under the write lock (two first opens must not both upgrade), a new file gets the tables, a version-3 or version-4 file is rebuilt in one step, and an older or newer file is refused without a write. Input: the connection. Returns the refusal or the SQLite error; a failed rebuild rolls back whole.
fn setup(conn: &Connection) -> Result<(), Failure> {
    let version = |c: &Connection| c.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0));
    if version(conn)? == LATEST {
        return Ok(());
    }
    in_write(conn, |c| {
        let table: bool = c.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'snippets'", [], |r| r.get(0))?;
        let upgrade = |insert: &str| format!("{SET_ASIDE}\n{TABLES}\n{}", COPY.replace("{insert}", insert));
        let step = match (version(c)?, table) {
            (v, _) if v > LATEST => return Err(Failure::Refused(NEWER.into())),
            (LATEST, true) => return Ok(()),
            (_, false) => TABLES.to_string(),
            (3, true) => upgrade("CASE insert_mode WHEN 'rich' THEN 'rich' ELSE 'paste' END"),
            (4, true) => upgrade("\"insert\""),
            _ => return Err(Failure::Refused(OLDER.into())),
        };
        c.execute_batch(&format!("{step}\nPRAGMA user_version = {LATEST};"))?;
        Ok(())
    })
}

/// Runs `f` between `begin` and COMMIT, rolling back when `f` or the commit fails. Inputs: the connection, the BEGIN statement and the work. Returns what `f` returns.
fn transaction<T, E: From<Failure>>(conn: &Connection, begin: &str, f: impl FnOnce(&Connection) -> Result<T, E>) -> Result<T, E> {
    conn.execute_batch(begin).map_err(|e| E::from(e.into()))?;
    let done = f(conn).and_then(|v| conn.execute_batch("COMMIT").map(|()| v).map_err(|e| E::from(e.into())));
    if done.is_err() {
        let _ = conn.execute_batch("ROLLBACK");
    }
    done
}

/// Runs `f` in a write transaction that takes the lock first (BEGIN IMMEDIATE), so a check made inside it still holds when it commits: two writers on two connections run one after the other. Input: the connection and the work. Returns what `f` returns; Err rolls everything back.
pub(super) fn in_write<T, E: From<Failure>>(conn: &Connection, f: impl FnOnce(&Connection) -> Result<T, E>) -> Result<T, E> {
    transaction(conn, "BEGIN IMMEDIATE", f)
}

/// Runs the reads of `f` in one read transaction, so they all see the same committed library: a folder deleted or an import committed between two of them cannot give old snippets with new folders. Input: the connection (not inside another transaction) and the reads. Returns what `f` returns.
pub fn snapshot<T, E: From<Failure>>(conn: &Connection, f: impl FnOnce(&Connection) -> Result<T, E>) -> Result<T, E> {
    transaction(conn, "BEGIN", f)
}

/// A database error as the message the page shows. Input: the error. Returns the message.
pub(super) fn fail(e: rusqlite::Error) -> String {
    Failure::from(e).into()
}

/// A stored list (one item per line) as its items, empty lines dropped. Input: the stored text. Returns the items.
pub(super) fn lines(text: String) -> Vec<String> {
    text.lines().filter(|l| !l.is_empty()).map(String::from).collect()
}

/// Reads every row of `sql` with `decode`; a row it cannot decode is named by its `kind` and id (column 0) so it can be fixed by hand, and nothing is changed. Inputs: the connection, the query, its parameters, the row kind and the decoder. Returns the rows.
pub(super) fn rows<T>(
    conn: &Connection,
    sql: &str,
    args: impl rusqlite::Params,
    kind: &str,
    decode: fn(&Row) -> rusqlite::Result<T>,
) -> Result<Vec<T>, Failure> {
    let mut stmt = conn.prepare(sql)?;
    let mut found = stmt.query(args)?;
    let mut out = Vec::new();
    while let Some(r) = found.next()? {
        out.push(decode(r).map_err(|e| {
            let id = r.get::<_, i64>(0).map_or("?".to_string(), |id| id.to_string());
            Failure::Refused(format!(
                "The snippets file holds a {kind} (row {id}) Cue cannot read: {e}. Nothing was changed; fix or delete that row with a SQLite editor."
            ))
        })?);
    }
    Ok(out)
}

/// One row of `columns` as a snippet. Input: the row. Returns the snippet, or Err for a value of the wrong type or an insert name serde does not know.
fn snippet(r: &Row) -> rusqlite::Result<Snippet> {
    Ok(Snippet {
        id: r.get(0)?,
        name: r.get(1)?,
        triggers: lines(r.get(2)?),
        body: r.get(3)?,
        uses: r.get(4)?,
        last_used: r.get(5)?,
        folder_id: r.get(6)?,
        enabled: r.get(7)?,
        insert: parse_name(&r.get::<_, String>(8)?).map_err(|e| rusqlite::Error::FromSqlConversionFailure(8, Type::Text, Box::new(e)))?,
        only_apps: lines(r.get(9)?),
        skip_apps: lines(r.get(10)?),
    })
}

/// Most used first, then by name.
pub fn list(conn: &Connection) -> Result<Vec<Snippet>, Failure> {
    select(conn, "body", [])
}

/// For the expander's index: a text longer than `max_body` bytes comes back empty (it is read when its trigger fires). `octet_length` of a column takes the size from the record header without reading the text's overflow pages, so a big text is never read just to be dropped.
pub fn list_for_index(conn: &Connection, max_body: usize) -> Result<Vec<Snippet>, Failure> {
    let max_body = i64::try_from(max_body).unwrap_or(i64::MAX);
    select(conn, "CASE WHEN octet_length(body) <= ?1 THEN body ELSE '' END", params![max_body])
}

/// Every snippet with its text left empty and never read: what a trigger check needs, so a save or a setting change does not load the whole library's texts. Input: the connection. Returns the snippets.
fn without_bodies(conn: &Connection) -> Result<Vec<Snippet>, Failure> {
    select(conn, "'' AS body", [])
}

/// Every snippet, most used first, with `body` (an SQL expression over the row and `args`) as its text. Inputs: the connection, the expression and its parameters. Returns the snippets.
fn select(conn: &Connection, body: &str, args: impl rusqlite::Params) -> Result<Vec<Snippet>, Failure> {
    rows(conn, &format!("SELECT {} FROM snippets ORDER BY uses DESC, name COLLATE NOCASE", columns(body)), args, "snippet", snippet)
}

/// The columns `snippet` reads, in its order, with `body` (an SQL expression) as the text. Input: the expression. Returns the column list.
fn columns(body: &str) -> String {
    format!("id, name, triggers, {body}, uses, last_used, folder_id, enabled, \"insert\", only_apps, skip_apps")
}

/// One snippet by id. Inputs: the connection and the id. Returns it, or None when there is none.
pub fn get(conn: &Connection, id: i64) -> Result<Option<Snippet>, Failure> {
    Ok(rows(conn, &format!("SELECT {} FROM snippets WHERE id = ?1", columns("body")), params![id], "snippet", snippet)?.pop())
}

/// The triggers of `all` that can fire under `policy`, each with its folder's programs and switch applied. Inputs: the snippets, the folders and the rules. Returns the triggers.
pub(super) fn resolve_all(all: &[Snippet], groups: &[Folder], policy: &Policy) -> Vec<Effective> {
    all.iter().flat_map(|s| policy.resolve(s, s.folder_id.and_then(|id| groups.iter().find(|f| f.id == id)))).collect()
}

/// Every trigger that can fire under the page's rules, with its folder's programs and switch applied; no text is read. Inputs: the connection (inside the caller's transaction) and the rules. Returns the triggers.
pub fn effective_all(conn: &Connection, policy: &Policy) -> Result<Vec<Effective>, Failure> {
    Ok(resolve_all(&without_bodies(conn)?, &folders(conn)?, policy))
}

/// How many snippets are stored. Input: the connection. Returns the count.
pub(super) fn count(conn: &Connection) -> Result<usize, Failure> {
    Ok(conn.query_row("SELECT COUNT(*) FROM snippets", [], |r| r.get::<_, i64>(0)).map(|n| n as usize)?)
}

/// Whether every enabled trigger of the library, under `policy`, can live with the others; no text is read. Returns Err naming the first pair that clashes (or the pattern limit); used inside a write transaction before a change switches triggers on wholesale (a folder switched on or deleted, the typing timing).
/// The whole library is checked, not only the pairs the change makes: writes never store a clash, so a clash found is this change's, except in a file edited by hand, where the change is refused naming a pair it did not make (accepted: the message names the pair to fix).
pub(super) fn check_library(conn: &Connection, policy: &Policy) -> Result<(), String> {
    let (all, groups) = (without_bodies(conn)?, folders(conn)?);
    let mut set = TriggerSet::default();
    all.iter().try_for_each(|s| set.try_add(&resolve_all(std::slice::from_ref(s), &groups, policy)))
}

/// A draft as a snippet row without an identity, for the rules that apply to it.
pub(super) fn snippet_of(d: &Draft) -> Snippet {
    Snippet {
        id: 0,
        name: d.name.clone(),
        triggers: d.triggers.clone(),
        body: String::new(),
        uses: 0,
        last_used: 0,
        folder_id: d.folder_id,
        enabled: d.enabled,
        insert: d.insert,
        only_apps: d.only_apps.clone(),
        skip_apps: d.skip_apps.clone(),
    }
}

/// Adds a snippet (`id` None) or replaces one. Input: the connection, the snippet to replace, the draft and the page's rules, which decide whether its triggers clash with the saved ones. Returns Err for a clash, the limit, a folder deleted meanwhile, a snippet deleted meanwhile, or a database failure.
pub fn save(conn: &Connection, id: Option<i64>, draft: &Draft, policy: &Policy) -> Result<(), String> {
    let d = clean(draft);
    check(&d)?;
    in_write(conn, |c| {
        let groups = folders(c)?;
        let folder = match d.folder_id {
            Some(fid) => Some(groups.iter().find(|f| f.id == fid).ok_or("That folder no longer exists. Reopen the Snippets page.")?),
            None => None,
        };
        let mine = policy.resolve(&snippet_of(&d), folder);
        TriggerSet::new(effective_all(c, policy)?.iter().filter(|o| Some(o.id) != id)).try_add(&mine)?;
        if id.is_none() && count(c)? >= MAX_SNIPPETS {
            return Err(format!("Cue keeps {MAX_SNIPPETS} snippets at most. Delete some first."));
        }
        write(c, id, &d)
    })
}

/// Stores a checked draft: replaces snippet `id`, or adds one with a new id when `id` is None. A replace that finds no such snippet (deleted meanwhile) is an error, never a silent new row.
pub(super) fn write(conn: &Connection, id: Option<i64>, d: &Draft) -> Result<(), String> {
    let (triggers, only_apps, skip_apps) = (d.triggers.join("\n"), d.only_apps.join("\n"), d.skip_apps.join("\n"));
    let sql = if id.is_some() {
        "UPDATE snippets SET name = :name, triggers = :triggers, body = :body, folder_id = :folder_id, enabled = :enabled, \"insert\" = :insert,
            only_apps = :only_apps, skip_apps = :skip_apps
         WHERE id = :id"
    } else {
        "INSERT INTO snippets (id, name, triggers, body, folder_id, enabled, \"insert\", only_apps, skip_apps)
         VALUES (:id, :name, :triggers, :body, :folder_id, :enabled, :insert, :only_apps, :skip_apps)"
    };
    let changed = conn
        .execute(
            sql,
            named_params! {
                ":id": id, ":name": d.name, ":triggers": triggers, ":body": d.body, ":folder_id": d.folder_id, ":enabled": d.enabled,
                ":insert": d.insert.name(), ":only_apps": only_apps, ":skip_apps": skip_apps,
            },
        )
        .map_err(fail)?;
    if changed == 0 {
        return Err("That snippet no longer exists. Reopen the Snippets page.".into());
    }
    Ok(())
}

/// Deletes one snippet; a missing id is not an error. Inputs: the connection and the id. Returns Err on a database failure.
pub fn remove(conn: &Connection, id: i64) -> Result<(), String> {
    conn.execute("DELETE FROM snippets WHERE id = ?1", params![id]).map_err(fail)?;
    Ok(())
}

/// One more use now, so the snippets used most come first. A deleted snippet is not an error: its id is never reused, so the count lands nowhere.
pub fn bump(conn: &Connection, id: i64) -> Result<(), String> {
    conn.execute("UPDATE snippets SET uses = uses + 1, last_used = CAST(strftime('%s', 'now') AS INTEGER) WHERE id = ?1", params![id])
        .map_err(fail)?;
    Ok(())
}
