//! Purpose: The folders of the snippet database: reading them, adding and changing one, deleting one (its snippets stay, unfiled) and finding or making one by name for an import.
//! Contents: folders — all folders; save_folder / remove_folder — writes; folder_named — the id of a folder by name, made when missing. Same file and rules as `store`.

use rusqlite::{named_params, params, Connection, Row};

use super::check::{check_folder, clean_folder};
use super::model::{Folder, FolderDraft};
use super::store::{fail, lines};

fn folder(r: &Row) -> rusqlite::Result<Folder> {
    Ok(Folder {
        id: r.get(0)?,
        name: r.get(1)?,
        enabled: r.get(2)?,
        prefix: r.get(3)?,
        only_apps: lines(r.get(4)?),
        skip_apps: lines(r.get(5)?),
    })
}

pub fn folders(conn: &Connection) -> Result<Vec<Folder>, String> {
    let mut stmt =
        conn.prepare("SELECT id, name, enabled, prefix, only_apps, skip_apps FROM folders ORDER BY name COLLATE NOCASE").map_err(fail)?;
    let rows = stmt.query_map([], folder).map_err(fail)?;
    rows.collect::<Result<_, _>>().map_err(fail)
}

/// Adds or replaces a folder (`id` None adds one).
pub fn save_folder(conn: &Connection, id: Option<i64>, draft: &FolderDraft) -> Result<i64, String> {
    let d = clean_folder(draft);
    check_folder(&d)?;
    let (only_apps, skip_apps) = (d.only_apps.join("\n"), d.skip_apps.join("\n"));
    let named = named_params! { ":id": id, ":name": d.name, ":enabled": d.enabled, ":prefix": d.prefix, ":only_apps": only_apps, ":skip_apps": skip_apps };
    conn.execute(
        "INSERT INTO folders (id, name, enabled, prefix, only_apps, skip_apps) VALUES (:id, :name, :enabled, :prefix, :only_apps, :skip_apps)
         ON CONFLICT(id) DO UPDATE SET name = :name, enabled = :enabled, prefix = :prefix, only_apps = :only_apps, skip_apps = :skip_apps",
        named,
    )
    .map_err(|e| if e.to_string().contains("UNIQUE") { "A folder with that name exists.".to_string() } else { fail(e) })?;
    Ok(id.unwrap_or_else(|| conn.last_insert_rowid()))
}

/// Deletes a folder; its snippets stay, unfiled.
pub fn remove_folder(conn: &Connection, id: i64) -> Result<(), String> {
    let tx = conn.unchecked_transaction().map_err(fail)?;
    conn.execute("UPDATE snippets SET folder_id = NULL WHERE folder_id = ?1", params![id]).map_err(fail)?;
    conn.execute("DELETE FROM folders WHERE id = ?1", params![id]).map_err(fail)?;
    tx.commit().map_err(fail)
}

/// The id of the folder with this name, made when there is none.
pub(super) fn folder_named(conn: &Connection, name: &str) -> Result<Option<i64>, String> {
    if name.trim().is_empty() {
        return Ok(None);
    }
    let found: Option<i64> = conn.query_row("SELECT id FROM folders WHERE name = ?1", params![name.trim()], |r| r.get(0)).ok();
    match found {
        Some(id) => Ok(Some(id)),
        None => save_folder(
            conn,
            None,
            &FolderDraft { name: name.into(), enabled: true, prefix: String::new(), only_apps: vec![], skip_apps: vec![] },
        )
        .map(Some),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snippets::model::Draft;
    use crate::snippets::store::{effective_all, list, open_memory, save};

    fn with_trigger(name: &str, trigger: &str) -> Draft {
        Draft { name: name.into(), body: "text".into(), triggers: vec![trigger.into()], ..Draft::default() }
    }

    #[test]
    fn folders_give_prefixes_and_deleting_one_keeps_its_snippets() {
        let conn = open_memory();
        let work = save_folder(
            &conn,
            None,
            &FolderDraft { name: "Work".into(), enabled: true, prefix: ";".into(), only_apps: vec![], skip_apps: vec![] },
        )
        .unwrap();
        save(&conn, None, &Draft { folder_id: Some(work), ..with_trigger("sig", "sig") }).unwrap();
        assert_eq!(effective_all(&conn).unwrap()[0].trigger, ";sig");
        assert!(save_folder(
            &conn,
            None,
            &FolderDraft { name: "work".into(), enabled: true, prefix: String::new(), only_apps: vec![], skip_apps: vec![] }
        )
        .is_err());
        remove_folder(&conn, work).unwrap();
        assert_eq!((list(&conn).unwrap()[0].folder_id, effective_all(&conn).unwrap()[0].trigger.as_str()), (None, "sig"));
    }
}
