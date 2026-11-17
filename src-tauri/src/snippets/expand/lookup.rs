//! Purpose: What a snippet's placeholders need from the saved snippets: the text of another snippet for `{snippet:name}` and the use count for `{counter}`. Reads the database only when the text asks for it.
//! Contents: Others — the snippets a text may include, found by name or trigger; counter — how many times a snippet has been used, plus one.

use tauri::AppHandle;

use super::super::model::Snippet;
use super::super::{store, with_db};

pub struct Others(Vec<Snippet>);

impl Others {
    /// Loads the snippets only when `wanted` (the text includes one).
    pub fn load(app: &AppHandle, wanted: bool) -> Others {
        let all = if wanted { with_db(app, store::list).unwrap_or_default() } else { Vec::new() };
        Others(all)
    }

    /// The text of the enabled snippet with this name or trigger, ignoring case.
    pub fn find(&self, name: &str) -> Option<String> {
        let same = |text: &String| text.eq_ignore_ascii_case(name);
        self.0.iter().find(|s| s.enabled && (same(&s.name) || s.triggers.iter().any(same))).map(|s| s.body.clone())
    }
}

/// What `{counter}` gives for snippet `id`; 1 when the text has no counter (`wanted` false) or the snippet is not saved yet.
pub fn counter(app: &AppHandle, id: Option<i64>, wanted: bool) -> i64 {
    let Some(id) = id.filter(|_| wanted) else { return 1 };
    with_db(app, |c| store::get(c, id)).ok().flatten().map_or(1, |s| s.uses + 1)
}
