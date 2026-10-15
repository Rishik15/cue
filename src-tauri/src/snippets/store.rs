//! Purpose: The snippet database: a SQLite file in the app data folder, opened for one operation and closed again, so nothing is resident while idle.
//! Contents: open — the file (created and brought up to date on first use); list / get / effective_all — reads (the folders are in `folders`); save / remove / bump / add_all — snippet writes.
//! Every statement uses bound parameters; text is never formatted into SQL. The columns `keyword`, `folder` and `paste` are from earlier versions and no longer read.

use std::path::Path;

use rusqlite::{named_params, params, Connection, Row};

use super::check::{check, clash, clean, MAX_SNIPPETS};
use super::folders::{folder_named, folders};
use super::model::{effective, Draft, Effective, Insert, Mode, Snippet};

/// The file version `setup` brings a database to.
const LATEST: i64 = 2;
const COLUMNS: &str = "id, name, triggers, body, uses, last_used, folder_id, mode, match_case, enabled, insert_mode, ending, inside_word, propagate_case, only_apps, skip_apps";

pub fn open(path: &Path) -> Result<Connection, String> {
    let existed = path.exists();
    let conn = Connection::open(path).map_err(|e| format!("Could not open the snippets file: {e}"))?;
    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap_or(0);
    if existed && version < LATEST {
        let _ = std::fs::copy(path, path.with_extension("backup.db")); // the file is about to be changed in place: keep the old one beside it
    }
    setup(&conn)?;
    Ok(conn)
}

/// Creates the table and adds what came after the first version (`user_version` says which steps are done).
fn setup(conn: &Connection) -> Result<(), String> {
    let prepare = |e: rusqlite::Error| format!("Could not prepare the snippets file: {e}");
    conn.execute_batch("CREATE TABLE IF NOT EXISTS snippets (id INTEGER PRIMARY KEY, name TEXT NOT NULL, keyword TEXT NOT NULL DEFAULT '', body TEXT NOT NULL, uses INTEGER NOT NULL DEFAULT 0)")
        .map_err(prepare)?;
    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).map_err(prepare)?;
    if version < 1 {
        conn.execute_batch(
            "BEGIN;
             ALTER TABLE snippets ADD COLUMN folder TEXT NOT NULL DEFAULT '';
             ALTER TABLE snippets ADD COLUMN mode TEXT NOT NULL DEFAULT 'auto';
             ALTER TABLE snippets ADD COLUMN match_case INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE snippets ADD COLUMN enabled INTEGER NOT NULL DEFAULT 1;
             ALTER TABLE snippets ADD COLUMN paste INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE snippets ADD COLUMN skip_apps TEXT NOT NULL DEFAULT '';
             PRAGMA user_version = 1;
             COMMIT;",
        )
        .map_err(prepare)?;
    }
    if version < 2 {
        conn.execute_batch(
            "BEGIN;
             CREATE TABLE IF NOT EXISTS folders (id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE COLLATE NOCASE, enabled INTEGER NOT NULL DEFAULT 1,
                prefix TEXT NOT NULL DEFAULT '', only_apps TEXT NOT NULL DEFAULT '', skip_apps TEXT NOT NULL DEFAULT '');
             ALTER TABLE snippets ADD COLUMN folder_id INTEGER;
             ALTER TABLE snippets ADD COLUMN triggers TEXT NOT NULL DEFAULT '';
             ALTER TABLE snippets ADD COLUMN last_used INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE snippets ADD COLUMN insert_mode TEXT NOT NULL DEFAULT 'auto';
             ALTER TABLE snippets ADD COLUMN ending TEXT NOT NULL DEFAULT '';
             ALTER TABLE snippets ADD COLUMN inside_word INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE snippets ADD COLUMN propagate_case INTEGER NOT NULL DEFAULT 1;
             ALTER TABLE snippets ADD COLUMN only_apps TEXT NOT NULL DEFAULT '';
             INSERT OR IGNORE INTO folders (name) SELECT DISTINCT folder FROM snippets WHERE folder != '';
             UPDATE snippets SET folder_id = (SELECT id FROM folders WHERE name = snippets.folder COLLATE NOCASE) WHERE folder != '';
             UPDATE snippets SET triggers = keyword, insert_mode = CASE WHEN paste = 1 THEN 'paste' ELSE 'auto' END;
             PRAGMA user_version = 2;
             COMMIT;",
        )
        .map_err(prepare)?;
    }
    Ok(())
}

pub(super) fn fail(e: rusqlite::Error) -> String {
    format!("Snippets: {e}")
}

pub(super) fn lines(text: String) -> Vec<String> {
    text.lines().filter(|l| !l.is_empty()).map(String::from).collect()
}

/// An empty database in memory, for tests.
#[cfg(test)]
pub(super) fn open_memory() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    setup(&conn).unwrap();
    conn
}

fn snippet(r: &Row) -> rusqlite::Result<Snippet> {
    Ok(Snippet {
        id: r.get(0)?,
        name: r.get(1)?,
        triggers: lines(r.get(2)?),
        body: r.get(3)?,
        uses: r.get(4)?,
        last_used: r.get(5)?,
        folder_id: r.get(6)?,
        mode: Mode::parse(&r.get::<_, String>(7)?),
        match_case: r.get(8)?,
        enabled: r.get(9)?,
        insert: Insert::parse(&r.get::<_, String>(10)?),
        ending: r.get(11)?,
        inside_word: r.get(12)?,
        propagate_case: r.get(13)?,
        only_apps: lines(r.get(14)?),
        skip_apps: lines(r.get(15)?),
    })
}

/// Most used first, then by name.
pub fn list(conn: &Connection) -> Result<Vec<Snippet>, String> {
    let mut stmt = conn.prepare(&format!("SELECT {COLUMNS} FROM snippets ORDER BY uses DESC, name COLLATE NOCASE")).map_err(fail)?;
    let rows = stmt.query_map([], snippet).map_err(fail)?;
    rows.collect::<Result<_, _>>().map_err(fail)
}

pub fn get(conn: &Connection, id: i64) -> Result<Option<Snippet>, String> {
    let mut stmt = conn.prepare(&format!("SELECT {COLUMNS} FROM snippets WHERE id = ?1")).map_err(fail)?;
    let mut rows = stmt.query_map(params![id], snippet).map_err(fail)?;
    rows.next().transpose().map_err(fail)
}

/// Every trigger that can fire, with its folder's prefix, programs and switch applied.
pub fn effective_all(conn: &Connection) -> Result<Vec<Effective>, String> {
    let (all, groups) = (list(conn)?, folders(conn)?);
    Ok(all.iter().flat_map(|s| effective(s, s.folder_id.and_then(|id| groups.iter().find(|f| f.id == id)))).collect())
}

fn count(conn: &Connection) -> Result<usize, String> {
    conn.query_row("SELECT COUNT(*) FROM snippets", [], |r| r.get::<_, i64>(0)).map(|n| n as usize).map_err(fail)
}

/// The other snippets' triggers that cannot live next to this one's.
fn refuse_clash(conn: &Connection, id: Option<i64>, d: &Draft) -> Result<(), String> {
    if d.triggers.is_empty() || !d.enabled {
        return Ok(());
    }
    let groups = folders(conn)?;
    let mine = effective(&snippet_of(d), d.folder_id.and_then(|fid| groups.iter().find(|f| f.id == fid)));
    for other in effective_all(conn)?.iter().filter(|o| Some(o.id) != id) {
        if let Some(m) = mine.iter().find(|m| clash((&m.trigger, m.mode), (&other.trigger, other.mode))) {
            return Err(format!(
                "The trigger {} clashes with {}: the same text, or one that would expand before the other can be typed.",
                m.trigger, other.trigger
            ));
        }
    }
    Ok(())
}

/// A draft as a snippet row without an identity, for the rules that apply to it.
fn snippet_of(d: &Draft) -> Snippet {
    Snippet {
        id: 0,
        name: d.name.clone(),
        triggers: d.triggers.clone(),
        body: String::new(),
        uses: 0,
        last_used: 0,
        folder_id: d.folder_id,
        mode: d.mode,
        match_case: d.match_case,
        enabled: d.enabled,
        insert: d.insert,
        ending: d.ending.clone(),
        inside_word: d.inside_word,
        propagate_case: d.propagate_case,
        only_apps: d.only_apps.clone(),
        skip_apps: d.skip_apps.clone(),
    }
}

/// Adds a snippet (`id` None) or replaces one.
pub fn save(conn: &Connection, id: Option<i64>, draft: &Draft) -> Result<(), String> {
    let d = clean(draft);
    check(&d)?;
    refuse_clash(conn, id, &d)?;
    if id.is_none() && count(conn)? >= MAX_SNIPPETS {
        return Err(format!("Cue keeps {MAX_SNIPPETS} snippets at most. Delete some first."));
    }
    let (triggers, only_apps, skip_apps, mode, insert) =
        (d.triggers.join("\n"), d.only_apps.join("\n"), d.skip_apps.join("\n"), d.mode.name(), d.insert.name());
    conn.execute(
        "INSERT INTO snippets (id, name, triggers, body, folder_id, mode, match_case, enabled, insert_mode, ending, inside_word, propagate_case, only_apps, skip_apps)
         VALUES (:id, :name, :triggers, :body, :folder_id, :mode, :match_case, :enabled, :insert_mode, :ending, :inside_word, :propagate_case, :only_apps, :skip_apps)
         ON CONFLICT(id) DO UPDATE SET name = :name, triggers = :triggers, body = :body, folder_id = :folder_id, mode = :mode, match_case = :match_case, enabled = :enabled,
            insert_mode = :insert_mode, ending = :ending, inside_word = :inside_word, propagate_case = :propagate_case, only_apps = :only_apps, skip_apps = :skip_apps",
        named_params! {
            ":id": id, ":name": d.name, ":triggers": triggers, ":body": d.body, ":folder_id": d.folder_id, ":mode": mode, ":match_case": d.match_case,
            ":enabled": d.enabled, ":insert_mode": insert, ":ending": d.ending, ":inside_word": d.inside_word, ":propagate_case": d.propagate_case,
            ":only_apps": only_apps, ":skip_apps": skip_apps,
        },
    )
    .map_err(fail)?;
    Ok(())
}

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

/// Adds the drafts that are not already stored (same name and text), each into the folder it names (made when missing); a draft that does not pass the limits or whose trigger clashes is skipped.
/// Returns how many were added.
pub fn add_all(conn: &Connection, entries: &[(Draft, String)]) -> Result<usize, String> {
    // One transaction: a failure half-way leaves nothing behind, and thousands of rows are not thousands of disk syncs.
    let tx = conn.unchecked_transaction().map_err(fail)?;
    let mut added = 0;
    for (d, folder) in entries {
        let dup: i64 = conn
            .query_row("SELECT COUNT(*) FROM snippets WHERE name = ?1 AND body = ?2", params![d.name.trim(), d.body], |r| r.get(0))
            .map_err(fail)?;
        let Ok(folder_id) = folder_named(conn, folder) else { continue };
        if dup == 0 && save(conn, None, &Draft { folder_id, ..d.clone() }).is_ok() {
            added += 1;
        }
    }
    tx.commit().map_err(fail)?;
    Ok(added)
}

#[cfg(test)]
mod tests;
