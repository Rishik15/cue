//! File: snippets/bulk.rs
//! Purpose: Adding many snippets at once (an import), checked against what is stored so a large file costs one pass over the database, not one per entry.
//! Contents: add_all — stores the drafts that are new, valid and clash-free, each into the folder it names (made when missing, at most `MAX_FOLDERS`), with includes of saved snippets that read the environment made literal, and stops at the snippet limit.

use std::collections::{HashMap, HashSet};

use rusqlite::Connection;

use super::check::{check, clean, TriggerSet, MAX_SNIPPETS};
use super::expand::lookup::names;
use super::folders::{folder_named, folders, new_folder};
use super::model::Draft;
use super::policy::Policy;
use super::store::{count, effective_all, in_write, snippet_of, write};
use super::transfer::escape_includes;

/// Most folders an import may leave behind; a file that names a different folder per entry must not bury the page.
pub const MAX_FOLDERS: usize = 100;

/// Adds the drafts that are not already stored (same name and text), valid, and whose triggers do not clash under the page's rules. A `{snippet:X}` whose X in the library as it was before the import reads the environment is stored as literal text. Returns how many were added (the rest are the caller's skipped) and how many includes of the added ones were made literal.
/// One transaction (`in_write`): a failure half-way leaves nothing behind, and thousands of rows are not thousands of disk syncs. A folder is made only for a draft that passed everything else.
pub fn add_all(conn: &Connection, entries: &[(Draft, String)], policy: &Policy) -> Result<(usize, usize), String> {
    in_write(conn, |conn| add_checked(conn, entries, policy))
}

/// The body of `add_all`, inside its transaction.
fn add_checked(conn: &Connection, entries: &[(Draft, String)], policy: &Policy) -> Result<(usize, usize), String> {
    let mut room = MAX_SNIPPETS.saturating_sub(count(conn)?);
    let mut set = TriggerSet::new(&effective_all(conn, policy)?);
    let mut groups = folders(conn)?;
    let library = super::store::list(conn)?;
    let (ids, bodies) = (names(&library), library.iter().map(|s| (s.id, &s.body)).collect::<HashMap<_, _>>());
    let find = |name: &str| ids.get(&name.to_ascii_lowercase()).and_then(|ids| bodies.get(ids.first()?)).map(|body| body.to_string());
    let mut known: HashSet<(String, String)> = library.iter().map(|s| (s.name.clone(), s.body.clone())).collect();
    let (mut added, mut escaped) = (0, 0);
    for (draft, folder) in entries {
        if room == 0 {
            break;
        }
        let mut d = clean(draft);
        let (body, made_literal) = escape_includes(&d.body, &find);
        d.body = body;
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
        (added, escaped, room) = (added + 1, escaped + made_literal, room - 1);
    }
    Ok((added, escaped))
}
