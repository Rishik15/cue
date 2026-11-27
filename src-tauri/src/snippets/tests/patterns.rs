//! File: snippets/tests/patterns.rs
//! Purpose: Protect the bound on `re:` triggers: every typed key is tried against each one, so at most `MAX_PATTERNS` enabled ones are stored, whichever way they arrive (save, import, switching a folder on).
//! Contents: scenario tests over a real SQLite file, and a hand-edited file past the bound refused by the expander's index with a message instead of a silent skip.

use super::helpers::{draft, open_db, save};
use crate::snippets::bulk::add_all;
use crate::snippets::check::MAX_PATTERNS;
use crate::snippets::expand::Index;
use crate::snippets::model::{Draft, FolderDraft};
use crate::snippets::policy::Policy;
use crate::snippets::{folders, store};

/// A snippet with one pattern trigger, numbered so no two clash.
fn pattern(i: usize) -> Draft {
    draft(&format!("Pattern {i}"), &[&format!(r"re:k{i}\d")], "x")
}

/// The 51st enabled pattern is refused with the count; a disabled one does not count, and editing a stored one at the limit is fine.
#[test]
fn the_fifty_first_pattern_is_refused_on_save() {
    let (_dir, conn) = open_db();
    (0..MAX_PATTERNS).for_each(|i| save(&conn, None, &pattern(i)).expect("within the limit"));
    let Err(message) = save(&conn, None, &pattern(MAX_PATTERNS)) else { panic!("51st accepted") };
    assert!(message.contains("51"), "{message}");
    save(&conn, None, &Draft { enabled: false, ..pattern(MAX_PATTERNS) }).expect("disabled does not count");
    let first = store::list(&conn).expect("list").into_iter().find(|s| s.name == "Pattern 0").expect("first");
    save(&conn, Some(first.id), &Draft { body: "y".into(), ..pattern(0) }).expect("edit at the limit");
}

/// An import skips the patterns past the limit and still adds plain snippets.
#[test]
fn an_import_skips_patterns_past_the_limit() {
    let (_dir, conn) = open_db();
    let entries: Vec<_> =
        (0..=MAX_PATTERNS).map(|i| (pattern(i), String::new())).chain([(draft("Plain", &[":p"], "x"), String::new())]).collect();
    assert_eq!(add_all(&conn, &entries, &Policy::default()), Ok((MAX_PATTERNS + 1, 0)));
    assert_eq!(store::list(&conn).expect("list").len(), MAX_PATTERNS + 1);
}

/// Switching a folder on counts its patterns: past the limit the folder stays off.
#[test]
fn enabling_a_folder_refuses_patterns_past_the_limit() {
    let (_dir, conn) = open_db();
    let policy = Policy::default();
    let off = FolderDraft { name: "Off".into(), enabled: false, only_apps: vec![], skip_apps: vec![] };
    let id = folders::save_folder(&conn, None, &off, &policy).expect("folder");
    (0..MAX_PATTERNS - 1).for_each(|i| save(&conn, None, &pattern(i)).expect("outside"));
    for i in MAX_PATTERNS..MAX_PATTERNS + 2 {
        save(&conn, None, &Draft { folder_id: Some(id), ..pattern(i) }).expect("inside the off folder");
    }
    let Err(message) = folders::save_folder(&conn, Some(id), &FolderDraft { enabled: true, ..off }, &policy) else {
        panic!("enabled past the limit")
    };
    assert!(message.contains("51"), "{message}");
    assert!(!folders::folders(&conn).expect("folders")[0].enabled);
}

/// A file edited by hand to hold more enabled patterns than writes allow is refused by the expander's index with the count, so nothing fires silently short of what the file says.
#[test]
fn a_hand_edited_file_past_the_limit_is_refused_with_a_message() {
    let (_dir, conn) = open_db();
    (0..MAX_PATTERNS).for_each(|i| save(&conn, None, &pattern(i)).expect("within the limit"));
    save(&conn, None, &Draft { enabled: false, ..pattern(MAX_PATTERNS) }).expect("disabled");
    assert!(Index::read(&conn).is_ok());
    conn.execute("UPDATE snippets SET enabled = 1", []).expect("hand edit");
    let Err(message) = Index::read(&conn) else { panic!("51 patterns indexed") };
    assert!(message.contains("51"), "{message}");
}

/// A hand-edited `re:` trigger that does not compile is counted in the index's message instead of being dropped silently; valid ones still index.
#[test]
fn a_hand_edited_pattern_that_does_not_compile_is_reported() {
    let (_dir, conn) = open_db();
    (0..2).for_each(|i| save(&conn, None, &pattern(i)).expect("valid"));
    assert_eq!(Index::read(&conn).expect("index").broken_note(), None);
    conn.execute("UPDATE snippets SET triggers = replace(triggers, 're:k0', 're:(') WHERE name = 'Pattern 0'", []).expect("hand edit");
    let note = Index::read(&conn).expect("index").broken_note().expect("the broken pattern is reported");
    assert!(note.starts_with("1 pattern"), "{note}");
}
