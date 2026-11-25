//! File: snippets/expand/lookup.rs
//! Purpose: What a snippet's placeholders need from the saved snippets: the names a `{snippet:name}` include is found by (ASCII case ignored), and the live use count for `{counter}`.
//! Contents: names — every enabled snippet's name and triggers, lowercased, to the ids that carry it; counter — how many times a snippet has been used, plus one (read from the database only when the text asks for it, after any claimed use-count write, see `count`).

use std::collections::HashMap;

use tauri::AppHandle;

use super::super::model::Snippet;
use super::super::{store, with_db};
use super::count;

/// The include names of `snippets`: each enabled snippet's name and triggers, lowercased in ASCII only (as the import safety walk in `transfer` compares them), to the ids that carry it. Input: the snippets in `store::list` order (`uses DESC, name`). Returns the map; the ids of a shared name keep that order, each id once.
pub fn names(snippets: &[Snippet]) -> HashMap<String, Vec<i64>> {
    let mut map: HashMap<String, Vec<i64>> = HashMap::new();
    for s in snippets.iter().filter(|s| s.enabled) {
        for name in std::iter::once(&s.name).chain(&s.triggers) {
            let ids = map.entry(name.to_ascii_lowercase()).or_default();
            if ids.last() != Some(&s.id) {
                ids.push(s.id);
            }
        }
    }
    map
}

/// What `{counter}` gives for snippet `id`; 1 when the text has no counter (`wanted` false) or the snippet is not saved yet.
pub fn counter(app: &AppHandle, id: Option<i64>, wanted: bool) -> i64 {
    let Some(id) = id.filter(|_| wanted) else { return 1 };
    count::read(|| with_db(app, |c| store::get(c, id))).ok().flatten().map_or(1, |s| s.uses + 1)
}
