//! Purpose: Adding many snippets at once (an import), checked against what is stored so a large file costs one pass over the database, not one per entry.
//! Contents: add_all — stores the drafts that are new, valid and clash-free, each into the folder it names (made when missing, at most `MAX_FOLDERS`), and stops at the snippet limit.

use std::collections::HashSet;

use rusqlite::Connection;

use super::check::{check, clean, MAX_SNIPPETS};
use super::folders::{folder_named, folders};
use super::model::{effective, Draft};
use super::store::{conflict, count, effective_all, fail, snippet_of, write};

/// Most folders an import may leave behind; a file that names a different folder per entry must not bury the page.
pub const MAX_FOLDERS: usize = 100;

/// Adds the drafts that are not already stored (same name and text), valid, and whose triggers do not clash. Returns how many were added; the rest are the caller's skipped.
/// One transaction: a failure half-way leaves nothing behind, and thousands of rows are not thousands of disk syncs. A folder is made only for a draft that passed everything else.
pub fn add_all(conn: &Connection, entries: &[(Draft, String)]) -> Result<usize, String> {
    let tx = conn.unchecked_transaction().map_err(fail)?;
    let mut room = MAX_SNIPPETS.saturating_sub(count(conn)?);
    let mut live = effective_all(conn)?;
    let mut groups = folders(conn)?;
    let mut known: HashSet<(String, String)> = super::store::list(conn)?.into_iter().map(|s| (s.name, s.body)).collect();
    let mut added = 0;
    for (draft, folder) in entries {
        if room == 0 {
            break;
        }
        let mut d = clean(draft);
        let name = folder.trim();
        let existing = groups.iter().find(|f| f.name.eq_ignore_ascii_case(name)).cloned();
        let fresh = !name.is_empty() && existing.is_none();
        if check(&d).is_err() || known.contains(&(d.name.clone(), d.body.clone())) || (fresh && groups.len() >= MAX_FOLDERS) {
            continue;
        }
        let mine = if d.enabled { effective(&snippet_of(&d), existing.as_ref()) } else { Vec::new() };
        if conflict(&mine, &live, None).is_err() {
            continue;
        }
        let Ok(folder_id) = folder_named(conn, name) else { continue };
        if fresh {
            groups = folders(conn)?;
        }
        d.folder_id = folder_id;
        write(conn, None, &d)?;
        known.insert((d.name.clone(), d.body.clone()));
        live.extend(mine);
        (added, room) = (added + 1, room - 1);
    }
    tx.commit().map_err(fail)?;
    Ok(added)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snippets::store::{list, open_memory, save};

    fn draft(name: &str, body: &str, trigger: &str) -> Draft {
        let triggers = if trigger.is_empty() { vec![] } else { vec![trigger.into()] };
        Draft { name: name.into(), body: body.into(), triggers, ..Draft::default() }
    }

    #[test]
    fn adding_many_skips_duplicates_and_makes_folders() {
        let conn = open_memory();
        save(&conn, None, &draft("a", "one", "")).unwrap();
        let entries = [(draft("a", "one", ""), String::new()), (draft("c", "three", ""), "Imported".into())];
        assert_eq!(add_all(&conn, &entries).unwrap(), 1);
        assert_eq!((list(&conn).unwrap().len(), folders(&conn).unwrap().len()), (2, 1));
    }

    #[test]
    fn an_entry_that_is_refused_makes_no_folder_and_clashes_within_the_file_are_caught() {
        let conn = open_memory();
        let bad = draft("", "no name", "");
        let entries = [
            (bad, "Ghost".into()),
            (draft("x", "one", ":sig"), String::new()),
            (draft("y", "two", ":SIG"), "Late".into()),
            (draft("x", "one", ":sig"), String::new()),
        ];
        assert_eq!(add_all(&conn, &entries).unwrap(), 1);
        assert!(folders(&conn).unwrap().is_empty());
    }

    #[test]
    fn folders_and_snippets_stop_at_their_caps() {
        let conn = open_memory();
        let many: Vec<_> = (0..MAX_FOLDERS + 5).map(|i| (draft(&format!("n{i}"), "t", ""), format!("F{i}"))).collect();
        assert_eq!(add_all(&conn, &many).unwrap(), MAX_FOLDERS);
        assert_eq!(folders(&conn).unwrap().len(), MAX_FOLDERS);
        let conn = open_memory();
        let flood: Vec<_> = (0..MAX_SNIPPETS + 10).map(|i| (draft(&format!("n{i}"), "t", ""), String::new())).collect();
        assert_eq!(add_all(&conn, &flood).unwrap(), MAX_SNIPPETS);
    }
}
