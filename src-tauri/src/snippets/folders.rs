//! File: snippets/folders.rs
//! Purpose: The folders of the snippet database: reading them, adding and changing one, deleting one (its snippets stay, unfiled) and finding or making one by name for an import.
//! Contents: folders — all folders; save_folder / remove_folder — writes, each one transaction; folder_named — the id of a folder by name, made when missing (inside the caller's transaction). Same file and rules as `store`.

use rusqlite::{named_params, params, Connection, ErrorCode, OptionalExtension, Row};

use super::check::{check_folder, clean_folder};
use super::model::{Folder, FolderDraft};
use super::policy::Policy;
use super::store::{check_library, fail, in_write, lines};

/// One folder from a row of `id, name, enabled, only_apps, skip_apps`. Input: the row. Returns the folder.
fn folder(r: &Row) -> rusqlite::Result<Folder> {
    Ok(Folder { id: r.get(0)?, name: r.get(1)?, enabled: r.get(2)?, only_apps: lines(r.get(3)?), skip_apps: lines(r.get(4)?) })
}

/// Every folder, by name with case ignored. Input: an open connection. Returns the folders or a user-readable error.
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
        let enabled = |id| c.query_row("SELECT enabled FROM folders WHERE id = ?1", params![id], |r| r.get(0)).optional().map_err(fail);
        let was_off = match id {
            Some(id) => enabled(id)? == Some(false),
            None => false,
        };
        let saved = write_folder(c, id, &d)?;
        if was_off && d.enabled {
            check_library(c, policy)?;
        }
        Ok(saved)
    })
}

/// Writes a checked folder draft, inside the caller's transaction. Returns the folder's id; a name taken by another folder (the only constraint a checked draft can break) is said in words, any other error as it is.
fn write_folder(conn: &Connection, id: Option<i64>, d: &FolderDraft) -> Result<i64, String> {
    let (only_apps, skip_apps) = (d.only_apps.join("\n"), d.skip_apps.join("\n"));
    let named = named_params! { ":id": id, ":name": d.name, ":enabled": d.enabled, ":only_apps": only_apps, ":skip_apps": skip_apps };
    conn.execute(
        "INSERT INTO folders (id, name, enabled, only_apps, skip_apps) VALUES (:id, :name, :enabled, :only_apps, :skip_apps)
         ON CONFLICT(id) DO UPDATE SET name = :name, enabled = :enabled, only_apps = :only_apps, skip_apps = :skip_apps",
        named,
    )
    .map_err(|e| match e.sqlite_error_code() {
        Some(ErrorCode::ConstraintViolation) => "A folder with that name exists.".to_string(),
        _ => fail(e),
    })?;
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

/// A new enabled folder draft for an imported name. Returns Err when `check_folder` refuses the name.
pub(super) fn new_folder(name: &str) -> Result<FolderDraft, String> {
    let d = FolderDraft { name: name.trim().into(), enabled: true, only_apps: vec![], skip_apps: vec![] };
    check_folder(&d).map(|()| d)
}

/// The id of the folder with this name, made when there is none. Inputs: the connection (inside the caller's transaction) and the name. Returns None for an empty name, or a user-readable error when the read or the write fails.
pub(super) fn folder_named(conn: &Connection, name: &str) -> Result<Option<i64>, String> {
    if name.trim().is_empty() {
        return Ok(None);
    }
    let found: Option<i64> =
        conn.query_row("SELECT id FROM folders WHERE name = ?1", params![name.trim()], |r| r.get(0)).optional().map_err(fail)?;
    match found {
        Some(id) => Ok(Some(id)),
        None => write_folder(conn, None, &new_folder(name)?).map(Some),
    }
}
