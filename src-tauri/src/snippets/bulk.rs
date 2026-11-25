//! Purpose: Adding many snippets at once (an import), checked against what is stored so a large file costs one pass over the database, not one per entry.
//! Contents: add_all — stores the drafts that are new, valid and clash-free, each into the folder it names (made when missing, at most `MAX_FOLDERS`), and stops at the snippet limit.

use std::collections::HashSet;

use rusqlite::Connection;

use super::check::{check, clean, TriggerSet, MAX_SNIPPETS};
use super::folders::{folder_named, folders, new_folder};
use super::model::Draft;
use super::policy::Policy;
use super::store::{count, effective_all, in_write, snippet_of, write};

/// Most folders an import may leave behind; a file that names a different folder per entry must not bury the page.
pub const MAX_FOLDERS: usize = 100;

/// Adds the drafts that are not already stored (same name and text), valid, and whose triggers do not clash under the page's rules. Returns how many were added; the rest are the caller's skipped.
/// One transaction (`in_write`): a failure half-way leaves nothing behind, and thousands of rows are not thousands of disk syncs. A folder is made only for a draft that passed everything else.
pub fn add_all(conn: &Connection, entries: &[(Draft, String)], policy: &Policy) -> Result<usize, String> {
    in_write(conn, |conn| add_checked(conn, entries, policy))
}

/// The body of `add_all`, inside its transaction.
fn add_checked(conn: &Connection, entries: &[(Draft, String)], policy: &Policy) -> Result<usize, String> {
    let mut room = MAX_SNIPPETS.saturating_sub(count(conn)?);
    let mut set = TriggerSet::new(&effective_all(conn, policy)?);
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
        if check(&d).is_err()
            || known.contains(&(d.name.clone(), d.body.clone()))
            || (fresh && (groups.len() >= MAX_FOLDERS || new_folder(name).is_err()))
        {
            continue;
        }
        let mine = if d.enabled { policy.resolve(&snippet_of(&d), existing.as_ref()) } else { Vec::new() };
        if set.try_add(&mine).is_err() {
            continue;
        }
        let Ok(folder_id) = folder_named(conn, name) else { continue };
        if fresh {
            groups = folders(conn)?;
        }
        d.folder_id = folder_id;
        write(conn, None, &d)?;
        known.insert((d.name.clone(), d.body.clone()));
        (added, room) = (added + 1, room - 1);
    }
    Ok(added)
}
