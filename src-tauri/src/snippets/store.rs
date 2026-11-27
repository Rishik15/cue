//! File: snippets/store.rs
//! Purpose: The snippet database: a SQLite file in the app data folder, opened for one operation and closed again, so nothing is resident while idle.
//! Contents: open — the file (created, or brought to the one schema by `setup`, on first use); in_write — the one write transaction every change runs in; check_library — whether the whole library's triggers can coexist; list / list_for_index / get / effective_all — reads (the folders are in `folders`); save / remove / bump — snippet writes (many at once are in `bulk`).
//! Every statement uses bound parameters; text is never formatted into SQL. The file has one schema (version 4); a 0.3 file (version 3) is rebuilt to it once, and an older or newer file is refused untouched.

use std::path::Path;
use std::time::Duration;

use rusqlite::types::Type;
use rusqlite::{named_params, params, Connection, Row};

use super::check::{check, clean, TriggerSet, MAX_SNIPPETS};
use super::folders::folders;
use super::model::{parse_name, Draft, Snippet};
use super::policy::{Effective, Policy};

/// The one schema of the file; `setup` creates it or brings a 0.3 file (version 3) to it.
const LATEST: i64 = 4;
const SNIPPETS: &str = "(id INTEGER PRIMARY KEY, name TEXT NOT NULL, triggers TEXT NOT NULL DEFAULT '', body TEXT NOT NULL, uses INTEGER NOT NULL DEFAULT 0,
    last_used INTEGER NOT NULL DEFAULT 0, folder_id INTEGER, enabled INTEGER NOT NULL DEFAULT 1,
    \"insert\" TEXT NOT NULL DEFAULT 'paste' CHECK (\"insert\" IN ('paste', 'rich')), only_apps TEXT NOT NULL DEFAULT '', skip_apps TEXT NOT NULL DEFAULT '')";
const FOLDERS: &str = "CREATE TABLE IF NOT EXISTS folders (id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE COLLATE NOCASE, enabled INTEGER NOT NULL DEFAULT 1,
    only_apps TEXT NOT NULL DEFAULT '', skip_apps TEXT NOT NULL DEFAULT '')";
/// Version 3 to 4: the table is rebuilt without the columns no version reads any more; folders keep their shape.
const UPGRADE: &str = "INSERT INTO snippets_new (id, name, triggers, body, uses, last_used, folder_id, enabled, \"insert\", only_apps, skip_apps)
    SELECT id, name, triggers, body, uses, last_used, folder_id, enabled, CASE insert_mode WHEN 'rich' THEN 'rich' ELSE 'paste' END, only_apps, skip_apps FROM snippets;
    DROP TABLE snippets;
    ALTER TABLE snippets_new RENAME TO snippets;";

/// Opens (and creates or upgrades) the snippets file. Input: its path. Returns the connection; a second writer waits up to 5 seconds for the lock instead of failing.
pub fn open(path: &Path) -> Result<Connection, String> {
    let conn = Connection::open(path).map_err(|e| format!("Could not open the snippets file: {e}"))?;
    conn.busy_timeout(Duration::from_secs(5)).map_err(fail)?;
    setup(&conn)?;
    Ok(conn)
}

/// Brings the file to the one schema: a file at it is only read; otherwise, under the write lock (two first opens must not both upgrade), a new file gets the tables, a version-3 file is rebuilt in one step, and an older or newer file is refused without a write. Input: the connection. Returns Err with the message for the page.
fn setup(conn: &Connection) -> Result<(), String> {
    let prepare = |e: rusqlite::Error| format!("Could not prepare the snippets file: {e}");
    let version = |c: &Connection| c.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0)).map_err(prepare);
    if version(conn)? == LATEST {
        return Ok(());
    }
    in_write(conn, |c| {
        let table =
            c.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'snippets'", [], |r| r.get::<_, bool>(0));
        let step = match (version(c)?, table.map_err(prepare)?) {
            (v, _) if v > LATEST => return Err("The snippets file was written by a newer Cue. Update Cue to use it.".into()),
            (LATEST, true) => return Ok(()),
            (_, false) => format!("CREATE TABLE snippets {SNIPPETS};"),
            (3, true) => format!("CREATE TABLE snippets_new {SNIPPETS}; {UPGRADE}"),
            _ => {
                return Err("This snippets file is from a Cue older than 0.2. Install Cue 0.3 first, run it once, then update again.".into())
            }
        };
        c.execute_batch(&format!("{step} {FOLDERS}; PRAGMA user_version = {LATEST};")).map_err(prepare)
    })
}

/// Runs `f` in a write transaction that takes the lock first (BEGIN IMMEDIATE), so a check made inside it still holds when it commits: two writers on two connections run one after the other. Input: the connection and the work. Returns what `f` returns; Err rolls everything back.
pub(super) fn in_write<T>(conn: &Connection, f: impl FnOnce(&Connection) -> Result<T, String>) -> Result<T, String> {
    conn.execute_batch("BEGIN IMMEDIATE").map_err(fail)?;
    let done = f(conn).and_then(|v| conn.execute_batch("COMMIT").map(|()| v).map_err(fail));
    if done.is_err() {
        let _ = conn.execute_batch("ROLLBACK");
    }
    done
}

/// A database error as the message the page shows. Input: the error. Returns the message.
pub(super) fn fail(e: rusqlite::Error) -> String {
    format!("Snippets: {e}")
}

/// A stored list (one item per line) as its items, empty lines dropped. Input: the stored text. Returns the items.
pub(super) fn lines(text: String) -> Vec<String> {
    text.lines().filter(|l| !l.is_empty()).map(String::from).collect()
}

/// One row of `columns` as a snippet. Input: the row. Returns the snippet, or Err for an insert name serde does not know (the CHECK keeps one from being stored).
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
pub fn list(conn: &Connection) -> Result<Vec<Snippet>, String> {
    select(conn, "body", [])
}

/// For the expander's index: a text longer than `max_body` bytes comes back empty (it is read when its trigger fires). `octet_length` of a column takes the size from the record header without reading the text's overflow pages, so a big text is never read just to be dropped.
pub fn list_for_index(conn: &Connection, max_body: usize) -> Result<Vec<Snippet>, String> {
    let max_body = i64::try_from(max_body).unwrap_or(i64::MAX);
    select(conn, "CASE WHEN octet_length(body) <= ?1 THEN body ELSE '' END", params![max_body])
}

/// Every snippet with its text left empty and never read: what a trigger check needs, so a save or a setting change does not load the whole library's texts. Input: the connection. Returns the snippets.
fn without_bodies(conn: &Connection) -> Result<Vec<Snippet>, String> {
    select(conn, "'' AS body", [])
}

/// Every snippet, most used first, with `body` (an SQL expression over the row and `args`) as its text. Inputs: the connection, the expression and its parameters. Returns the snippets.
fn select(conn: &Connection, body: &str, args: impl rusqlite::Params) -> Result<Vec<Snippet>, String> {
    let mut stmt =
        conn.prepare(&format!("SELECT {} FROM snippets ORDER BY uses DESC, name COLLATE NOCASE", columns(body))).map_err(fail)?;
    let rows = stmt.query_map(args, snippet).map_err(fail)?;
    rows.collect::<Result<_, _>>().map_err(fail)
}

/// The columns `snippet` reads, in its order, with `body` (an SQL expression) as the text. Input: the expression. Returns the column list.
fn columns(body: &str) -> String {
    format!("id, name, triggers, {body}, uses, last_used, folder_id, enabled, \"insert\", only_apps, skip_apps")
}

/// One snippet by id. Inputs: the connection and the id. Returns it, or None when there is none.
pub fn get(conn: &Connection, id: i64) -> Result<Option<Snippet>, String> {
    let mut stmt = conn.prepare(&format!("SELECT {} FROM snippets WHERE id = ?1", columns("body"))).map_err(fail)?;
    let mut rows = stmt.query_map(params![id], snippet).map_err(fail)?;
    rows.next().transpose().map_err(fail)
}

/// Every trigger that can fire under the page's rules, with its folder's programs and switch applied; no text is read. Inputs: the connection and the rules. Returns the triggers.
pub fn effective_all(conn: &Connection, policy: &Policy) -> Result<Vec<Effective>, String> {
    let (all, groups) = (without_bodies(conn)?, folders(conn)?);
    Ok(all.iter().flat_map(|s| policy.resolve(s, s.folder_id.and_then(|id| groups.iter().find(|f| f.id == id)))).collect())
}

/// How many snippets are stored. Input: the connection. Returns the count.
pub(super) fn count(conn: &Connection) -> Result<usize, String> {
    conn.query_row("SELECT COUNT(*) FROM snippets", [], |r| r.get::<_, i64>(0)).map(|n| n as usize).map_err(fail)
}

/// Whether every enabled trigger of the library, under `policy`, can live with the others; no text is read. Returns Err naming the first pair that clashes (or the pattern limit); used before a change switches triggers on wholesale (a folder, the typing timing).
/// The whole library is checked, not only the pairs the change makes: writes never store a clash, so a clash found is this change's, except in a file edited by hand, where the change is refused naming a pair it did not make (accepted: the message names the pair to fix).
pub(super) fn check_library(conn: &Connection, policy: &Policy) -> Result<(), String> {
    let (all, groups) = (without_bodies(conn)?, folders(conn)?);
    let mut set = TriggerSet::default();
    all.iter().try_for_each(|s| set.try_add(&policy.resolve(s, s.folder_id.and_then(|id| groups.iter().find(|f| f.id == id)))))
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

/// Adds a snippet (`id` None) or replaces one. Input: the connection, the snippet to replace, the draft and the page's rules, which decide whether its triggers clash with the saved ones.
pub fn save(conn: &Connection, id: Option<i64>, draft: &Draft, policy: &Policy) -> Result<(), String> {
    let d = clean(draft);
    check(&d)?;
    in_write(conn, |c| {
        let groups = folders(c)?;
        let mine = policy.resolve(&snippet_of(&d), d.folder_id.and_then(|fid| groups.iter().find(|f| f.id == fid)));
        TriggerSet::new(effective_all(c, policy)?.iter().filter(|o| Some(o.id) != id)).try_add(&mine)?;
        if id.is_none() && count(c)? >= MAX_SNIPPETS {
            return Err(format!("Cue keeps {MAX_SNIPPETS} snippets at most. Delete some first."));
        }
        write(c, id, &d)
    })
}

/// Stores a checked draft: replaces snippet `id`, or adds one when `id` is None. A replace that finds no such snippet (deleted meanwhile) is an error, never a silent new row.
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

/// One more use now, so the snippets used most come first.
pub fn bump(conn: &Connection, id: i64) -> Result<(), String> {
    conn.execute("UPDATE snippets SET uses = uses + 1, last_used = CAST(strftime('%s', 'now') AS INTEGER) WHERE id = ?1", params![id])
        .map_err(fail)?;
    Ok(())
}
