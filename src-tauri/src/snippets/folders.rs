//! File: snippets/folders.rs
//! Purpose: The folders of the snippet database: reading them, adding and changing one, deleting one (its snippets stay, unfiled) and finding or making one by name for an import.
//! Contents: folders — all folders; save_folder / remove_folder — writes, each one transaction that refuses a change whose triggers would clash; new_folder — the folder an import would make for a name; write_folder — one folder written inside the caller's transaction. Same file and rules as `store`.

use rusqlite::{named_params, params, Connection, ErrorCode, OptionalExtension, Row};

use super::check::{check_folder, clean_folder};
use super::model::{Folder, FolderDraft};
use super::policy::Policy;
use super::store::{check_library, fail, in_write, lines, rows, Failure};

const GONE: &str = "That folder no longer exists. Reopen the Snippets page.";

/// One folder from a row of `id, name, enabled, only_apps, skip_apps`. Input: the row. Returns the folder.
fn folder(r: &Row) -> rusqlite::Result<Folder> {
    Ok(Folder { id: r.get(0)?, name: r.get(1)?, enabled: r.get(2)?, only_apps: lines(r.get(3)?), skip_apps: lines(r.get(4)?) })
}

/// Every folder, by name with case ignored. Input: an open connection. Returns the folders, or the failure (a damaged row named).
pub fn folders(conn: &Connection) -> Result<Vec<Folder>, Failure> {
    rows(conn, "SELECT id, name, enabled, only_apps, skip_apps FROM folders ORDER BY name COLLATE NOCASE", [], "folder", folder)
}

/// Adds a folder (`id` None) or changes one. Switching a disabled folder on turns its snippets' triggers on, so the whole library is checked then (under the page's rules `policy`); a clash refuses the change and nothing is saved. Changing a folder deleted meanwhile is an error, never a new folder. Returns the folder's id.
pub fn save_folder(conn: &Connection, id: Option<i64>, draft: &FolderDraft, policy: &Policy) -> Result<i64, String> {
    let d = clean_folder(draft);
    check_folder(&d)?;
    in_write(conn, |c| {
        let was_off = match id {
            Some(id) => !c
                .query_row("SELECT enabled FROM folders WHERE id = ?1", params![id], |r| r.get::<_, bool>(0))
                .optional()
                .map_err(fail)?
                .ok_or(GONE)?,
            None => false,
        };
        let saved = write_folder(c, id, &d)?;
        if was_off && d.enabled {
            check_library(c, policy)?;
        }
        Ok(saved)
    })
}

/// Writes a checked folder draft, inside the caller's transaction: changes folder `id`, or adds one with a new id when `id` is None. Returns the folder's id; a name taken by another folder (the only constraint a checked draft can break) is said in words, a missing folder too, any other error as it is.
pub(super) fn write_folder(conn: &Connection, id: Option<i64>, d: &FolderDraft) -> Result<i64, String> {
    let (only_apps, skip_apps) = (d.only_apps.join("\n"), d.skip_apps.join("\n"));
    let named = named_params! { ":id": id, ":name": d.name, ":enabled": d.enabled, ":only_apps": only_apps, ":skip_apps": skip_apps };
    let sql = if id.is_some() {
        "UPDATE folders SET name = :name, enabled = :enabled, only_apps = :only_apps, skip_apps = :skip_apps WHERE id = :id"
    } else {
        "INSERT INTO folders (id, name, enabled, only_apps, skip_apps) VALUES (:id, :name, :enabled, :only_apps, :skip_apps)"
    };
    let changed = conn.execute(sql, named).map_err(|e| match e.sqlite_error_code() {
        Some(ErrorCode::ConstraintViolation) => "A folder with that name exists.".to_string(),
        _ => fail(e),
    })?;
    match id {
        Some(_) if changed == 0 => Err(GONE.into()),
        Some(id) => Ok(id),
        None => Ok(conn.last_insert_rowid()),
    }
}

/// Deletes a folder; its snippets stay, unfiled (the foreign key clears their folder). Unfiled, a disabled folder's snippets switch on, so the whole library is checked under the page's rules `policy` in the same transaction and a clash (or the pattern limit) refuses the deletion. A missing folder is not an error. Inputs: the connection, the folder and the rules.
pub fn remove_folder(conn: &Connection, id: i64, policy: &Policy) -> Result<(), String> {
    in_write(conn, |c| {
        c.execute("DELETE FROM folders WHERE id = ?1", params![id]).map_err(fail)?;
        check_library(c, policy)
    })
}

/// The folder an import names, as it would be made: from the file's definition of it (its switch and program lists) when the file has one, else enabled everywhere. Inputs: the name and the file's folders. Returns Err when `check_folder` refuses it.
pub(super) fn new_folder(name: &str, defined: &[FolderDraft]) -> Result<FolderDraft, String> {
    let name = name.trim();
    let d = match defined.iter().find(|f| f.name.trim().eq_ignore_ascii_case(name)) {
        Some(f) => clean_folder(&FolderDraft { name: name.into(), ..f.clone() }),
        None => FolderDraft { name: name.into(), enabled: true, only_apps: vec![], skip_apps: vec![] },
    };
    check_folder(&d).map(|()| d)
}
