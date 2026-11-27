//! File: snippets/bulk.rs
//! Purpose: Adding many snippets at once (an import), checked against what is stored so a large file costs one pass over the database, not one per entry.
//! Contents: add_all — stores the drafts that are new, valid and clash-free, each into the folder it names (a stored one, or one made from the file's definition of it with its switch and program lists, at most `MAX_FOLDERS`), with includes of saved snippets that read the environment made literal, and stops at the snippet limit; Home / home / stored — the folder an entry goes into.

use std::collections::{HashMap, HashSet};

use rusqlite::Connection;

use super::check::{check, clean, TriggerSet, MAX_SNIPPETS};
use super::expand::lookup::names;
use super::folders::{folders, new_folder, write_folder};
use super::model::{Draft, Folder, FolderDraft};
use super::policy::Policy;
use super::store::{count, effective_all, in_write, snippet_of, write};
use super::transfer::escape_includes;

/// Most folders an import may leave behind; a file that names a different folder per entry must not bury the page.
pub const MAX_FOLDERS: usize = 100;

/// Adds the drafts that are not already stored (same name and text), valid, and whose triggers do not clash under the page's rules, in the folder each names: the stored one with that name, else a new one from `defined` (the file's folders), so a disabled or program-bound folder comes back as it was and its triggers are judged as they will fire. A `{snippet:X}` where any enabled snippet named or triggered X in the library as it was before the import could read the environment (use counts may pick any of them later) is stored as literal text. Returns how many were added (the rest are the caller's skipped) and how many includes of the added ones were made literal.
/// One transaction (`in_write`): a failure half-way, a storage failure included, leaves nothing behind, and thousands of rows are not thousands of disk syncs. A folder is made only for a draft that passed everything else.
pub fn add_all(conn: &Connection, entries: &[(Draft, String)], defined: &[FolderDraft], policy: &Policy) -> Result<(usize, usize), String> {
    in_write(conn, |conn| add_checked(conn, entries, defined, policy))
}

/// Where an entry goes: no folder, a stored folder, or a folder to make.
enum Home {
    Unfiled,
    Stored(Folder),
    New(FolderDraft),
}

impl Home {
    /// The folder as the rules see it (a new one with no id yet). Returns it, None when unfiled.
    fn as_folder(&self) -> Option<Folder> {
        match self {
            Home::Unfiled => None,
            Home::Stored(f) => Some(f.clone()),
            Home::New(d) => Some(stored(0, d.clone())),
        }
    }
}

/// Folder draft `d` as the row it is stored as under `id`. Returns the folder.
fn stored(id: i64, d: FolderDraft) -> Folder {
    Folder { id, name: d.name, enabled: d.enabled, only_apps: d.only_apps, skip_apps: d.skip_apps }
}

/// The folder named `name` goes into: unfiled for an empty name, the stored one with that name (ASCII case ignored), else a new one from the file's definition. Inputs: the name, the stored folders and the file's. Returns None when a new folder may not be made (`check_folder` refuses it, or `MAX_FOLDERS` are stored).
fn home(name: &str, groups: &[Folder], defined: &[FolderDraft]) -> Option<Home> {
    let name = name.trim();
    if name.is_empty() {
        return Some(Home::Unfiled);
    }
    if let Some(f) = groups.iter().find(|f| f.name.eq_ignore_ascii_case(name)) {
        return Some(Home::Stored(f.clone()));
    }
    (groups.len() < MAX_FOLDERS).then(|| new_folder(name, defined).ok().map(Home::New)).flatten()
}

/// The body of `add_all`, inside its transaction. Skips only what the checks refuse; any storage failure is returned, so the whole import rolls back.
fn add_checked(conn: &Connection, entries: &[(Draft, String)], defined: &[FolderDraft], policy: &Policy) -> Result<(usize, usize), String> {
    let mut room = MAX_SNIPPETS.saturating_sub(count(conn)?);
    let mut set = TriggerSet::new(&effective_all(conn, policy)?);
    let mut groups = folders(conn)?;
    let library = super::store::list(conn)?;
    let (ids, bodies) = (names(&library), library.iter().map(|s| (s.id, s.body.as_str())).collect::<HashMap<_, _>>());
    let find =
        |name: &str| ids.get(&name.to_ascii_lowercase()).into_iter().flatten().filter_map(|id| bodies.get(id).copied()).collect::<Vec<_>>();
    let mut known: HashSet<(String, String)> = library.iter().map(|s| (s.name.clone(), s.body.clone())).collect();
    let (mut added, mut escaped) = (0, 0);
    for (draft, folder) in entries {
        if room == 0 {
            break;
        }
        let mut d = clean(draft);
        let (body, made_literal) = escape_includes(&d.body, &find);
        d.body = body;
        let Some(home) = home(folder, &groups, defined) else { continue };
        if check(&d).is_err() || known.contains(&(d.name.clone(), d.body.clone())) {
            continue;
        }
        let mine = if d.enabled { policy.resolve(&snippet_of(&d), home.as_folder().as_ref()) } else { Vec::new() };
        if set.try_add(&mine).is_err() {
            continue;
        }
        d.folder_id = match home {
            Home::Unfiled => None,
            Home::Stored(f) => Some(f.id),
            Home::New(f) => {
                let id = write_folder(conn, None, &f)?;
                groups.push(stored(id, f));
                Some(id)
            }
        };
        write(conn, None, &d)?;
        known.insert((d.name.clone(), d.body.clone()));
        (added, escaped, room) = (added + 1, escaped + made_literal, room - 1);
    }
    Ok((added, escaped))
}
