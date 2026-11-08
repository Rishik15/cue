//! File: snippets/tests/helpers.rs
//! Purpose: Builders shared by the snippet tests: drafts, a real database in a temp folder and typing triggers.
//! Contents: draft — a draft snippet; open_db — a fresh database; effective — a trigger as the matcher gets it.

use rusqlite::Connection;
use tempfile::TempDir;

use crate::snippets::model::{Draft, Effective, Mode};
use crate::snippets::store;

/// A snippet draft with default options. Inputs: name, triggers and text. Returns the draft.
pub fn draft(name: &str, triggers: &[&str], body: &str) -> Draft {
    Draft { name: name.into(), triggers: triggers.iter().map(|t| t.to_string()).collect(), body: body.into(), ..Draft::default() }
}

/// A new snippets database in a temp folder, opened the way the app opens it. Returns the folder (keep it alive) and the connection.
pub fn open_db() -> (TempDir, Connection) {
    let dir = tempfile::tempdir().expect("temp dir");
    let conn = store::open(&dir.path().join("snippets.db")).expect("open database");
    (dir, conn)
}

/// One trigger with default options. Inputs: snippet id and trigger. Returns it as the matcher is given it.
pub fn effective(id: i64, trigger: &str) -> Effective {
    Effective {
        id,
        trigger: trigger.into(),
        mode: Mode::Auto,
        match_case: false,
        inside_word: false,
        propagate_case: true,
        ending: String::new(),
        only_apps: Vec::new(),
        skip_apps: Vec::new(),
    }
}
