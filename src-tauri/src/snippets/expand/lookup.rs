//! File: snippets/expand/lookup.rs
//! Purpose: What a snippet's placeholders need from the saved snippets: the names a `{snippet:name}` include is found by (ASCII case ignored), and the live use count for `{counter}`.
//! Contents: names — every enabled snippet's name and triggers, lowercased, to its id; counter — how many times a snippet has been used, plus one (read from the database only when the text asks for it).

use std::collections::HashMap;

use tauri::AppHandle;

use super::super::model::Snippet;
use super::super::{store, with_db};

/// The include names of `snippets`: each enabled snippet's name and triggers, lowercased in ASCII only (as the import safety walk in `transfer` compares them), to its id. Input: the snippets in the order the first match wins (`uses DESC, name`, as `store::list` gives them). Returns the map; a name two snippets share keeps the first.
pub fn names(snippets: &[Snippet]) -> HashMap<String, i64> {
    let mut map = HashMap::new();
    for s in snippets.iter().filter(|s| s.enabled) {
        for name in std::iter::once(&s.name).chain(&s.triggers) {
            map.entry(name.to_ascii_lowercase()).or_insert(s.id);
        }
    }
    map
}

/// What `{counter}` gives for snippet `id`; 1 when the text has no counter (`wanted` false) or the snippet is not saved yet.
pub fn counter(app: &AppHandle, id: Option<i64>, wanted: bool) -> i64 {
    let Some(id) = id.filter(|_| wanted) else { return 1 };
    with_db(app, |c| store::get(c, id)).ok().flatten().map_or(1, |s| s.uses + 1)
}
