//! Purpose: The folders of the snippet database: reading them, adding and changing one, deleting one (its snippets stay, unfiled) and finding or making one by name for an import.
//! Contents: folders — all folders; save_folder / remove_folder — writes, each one transaction; folder_named — the id of a folder by name, made when missing (inside the caller's transaction). Same file and rules as `store`.

use rusqlite::{named_params, params, Connection, Row};

use super::check::{check_folder, clean_folder};
use super::model::{Folder, FolderDraft};
use super::policy::Policy;
use super::store::{check_library, fail, in_write, lines};

fn folder(r: &Row) -> rusqlite::Result<Folder> {
    Ok(Folder { id: r.get(0)?, name: r.get(1)?, enabled: r.get(2)?, only_apps: lines(r.get(3)?), skip_apps: lines(r.get(4)?) })
}

pub fn folders(conn: &Connection) -> Result<Vec<Folder>, String> {
    let mut stmt =
        conn.prepare("SELECT id, name, enabled, only_apps, skip_apps FROM folders ORDER BY name COLLATE NOCASE").map_err(fail)?;
    let rows = stmt.query_map([], folder).map_err(fail)?;
    rows.collect::<Result<_, _>>().map_err(fail)
}

/// Adds or replaces a folder (`id` None adds one). Switching a disabled folder on turns its snippets' triggers on, so the whole library is checked then (under the page's rules `policy`); a clash refuses the change and nothing is saved.
pub fn save_folder(conn: &Connection, id: Option<i64>, draft: &FolderDraft, policy: &Policy) -> Result<i64, String> {
    let d = clean_folder(draft);
    check_folder(&d)?;
    in_write(conn, |c| {
        let was_off = id.is_some_and(|id| c.query_row("SELECT enabled FROM folders WHERE id = ?1", params![id], |r| r.get(0)) == Ok(false));
        let saved = write_folder(c, id, &d)?;
        if was_off && d.enabled {
            check_library(c, policy)?;
        }
        Ok(saved)
    })
}

/// Writes a checked folder draft, inside the caller's transaction. Returns the folder's id.
fn write_folder(conn: &Connection, id: Option<i64>, d: &FolderDraft) -> Result<i64, String> {
    let (only_apps, skip_apps) = (d.only_apps.join("\n"), d.skip_apps.join("\n"));
    let named = named_params! { ":id": id, ":name": d.name, ":enabled": d.enabled, ":only_apps": only_apps, ":skip_apps": skip_apps };
    conn.execute(
        "INSERT INTO folders (id, name, enabled, only_apps, skip_apps) VALUES (:id, :name, :enabled, :only_apps, :skip_apps)
         ON CONFLICT(id) DO UPDATE SET name = :name, enabled = :enabled, only_apps = :only_apps, skip_apps = :skip_apps",
        named,
    )
    .map_err(|e| if e.to_string().contains("UNIQUE") { "A folder with that name exists.".to_string() } else { fail(e) })?;
    Ok(id.unwrap_or_else(|| conn.last_insert_rowid()))
}

/// Deletes a folder; its snippets stay, unfiled.
pub fn remove_folder(conn: &Connection, id: i64) -> Result<(), String> {
    in_write(conn, |c| {
        c.execute("UPDATE snippets SET folder_id = NULL WHERE folder_id = ?1", params![id]).map_err(fail)?;
        c.execute("DELETE FROM folders WHERE id = ?1", params![id]).map_err(fail)?;
        Ok(())
    })
}

/// The id of the folder with this name, made when there is none.
pub(super) fn folder_named(conn: &Connection, name: &str) -> Result<Option<i64>, String> {
    if name.trim().is_empty() {
        return Ok(None);
    }
    let found: Option<i64> = conn.query_row("SELECT id FROM folders WHERE name = ?1", params![name.trim()], |r| r.get(0)).ok();
    match found {
        Some(id) => Ok(Some(id)),
        None => write_folder(conn, None, &FolderDraft { name: name.into(), enabled: true, only_apps: vec![], skip_apps: vec![] }).map(Some),
    }
}
