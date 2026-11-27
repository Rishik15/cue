//! File: snippets/tests/helpers.rs
//! Purpose: Builders shared by the snippet tests: drafts, a real database in a temp folder and typing triggers.
//! Contents: draft — a draft snippet; open_db — a fresh database; save — a save under the page's default rules; with_mode — the default rules with a chosen timing; effective — a trigger as the matcher gets it; typed / typed_with — a body as the real expansion code types it, includes answered from the index of the saved library (fresh, or one kept by the caller); notepad — the fixed values those read.

use std::collections::HashMap;

use rusqlite::Connection;
use tempfile::TempDir;

use crate::platform::LocalTime;
use crate::snippets::expand::template::{expand, Context};
use crate::snippets::expand::Index;
use crate::snippets::model::{Draft, Mode};
use crate::snippets::policy::{Effective, Policy};
use crate::snippets::{keys, store};

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

/// Saves a draft under the page's default rules. Inputs: the connection, the snippet to replace (None adds) and the draft. Returns what `store::save` returns.
pub fn save(conn: &Connection, id: Option<i64>, d: &Draft) -> Result<(), String> {
    store::save(conn, id, d, &Policy::default())
}

/// The page's default rules with timing `mode`. Input: the mode. Returns the rules.
pub fn with_mode(mode: Mode) -> Policy {
    let value = serde_json::to_value(mode).expect("a mode has a name");
    Policy::default().changed(keys::MODE, &value).expect("the timing decides clashes")
}

/// One trigger with default options. Inputs: snippet id and trigger. Returns it as the matcher is given it.
pub fn effective(id: i64, trigger: &str) -> Effective {
    Effective {
        id,
        trigger: trigger.into(),
        when: Mode::Auto.when(trigger),
        match_case: false,
        inside_word: false,
        propagate_case: true,
        ending: String::new(),
        only_apps: Vec::new(),
        skip_apps: Vec::new(),
    }
}

/// What `body` types in Notepad through the real expansion code, with the saved snippets of `conn` as includes and every environment variable reading `TOPSECRET`. Inputs: the connection and the text. Returns the typed text.
pub fn typed(conn: &Connection, body: &str) -> String {
    typed_with(conn, &Index::read(conn).expect("index"), body)
}

/// `typed` with includes answered from `index`, one the caller keeps across expansions. Inputs: the connection (for texts not kept in memory), the index and the text. Returns the typed text.
pub fn typed_with(conn: &Connection, index: &Index, body: &str) -> String {
    let lookup = |name: &str| index.include(name, &|id| store::get(conn, id).ok().flatten().map(|s| s.body));
    expand(body, &notepad(&lookup)).text
}

/// The values a text reads in Notepad at a fixed moment, with every environment variable reading `TOPSECRET`. Input: the include lookup. Returns the context.
pub fn notepad<'a>(lookup: &'a dyn Fn(&str) -> Option<String>) -> Context<'a> {
    Context {
        now: LocalTime { year: 2026, month: 3, day: 5, hour: 14, minute: 7, second: 9, weekday: 4 },
        clipboard: None,
        app: "notepad".into(),
        counter: 1,
        lookup,
        env: &|_| Some("TOPSECRET".into()),
        entropy: &|| Some(0),
        args: HashMap::new(),
        captures: HashMap::new(),
    }
}
