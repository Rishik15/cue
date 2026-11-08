//! File: input/tests/saves.rs
//! Purpose: Protect saving and loading bindings: whole-set validation against pending writes, failed writes, reset, the bounded queue, and stored rows that are invalid at startup.
//! Contents: scenario tests of the catalog, `Input::reset` and recorded saves.

use std::time::Duration;

use super::{drain, fake, CAT};
use crate::input::fake::{Fake, FakeStore};
use crate::input::{Event, InputError, Key, Mods, Update};

fn catalog_of(f: &Fake) -> crate::input::CatalogView {
    match f.input.shortcuts().try_next().unwrap() {
        Update::Next(v) | Update::Resnapshot(v) => v,
    }
}

/// A queued but unfinished save counts: a second recording that clashes with it is refused before anything is written.
#[test]
fn validation_uses_the_pending_latest_set() {
    let f = fake();
    f.store.block();
    let owner = f.input.owner();
    let a = owner.record("grab").unwrap();
    f.mod_down(Mods::CTRL_L);
    f.down(Key::K);
    f.up(Key::K);
    drop(a);
    let b = owner.record("selection").unwrap();
    let sub = b.observe();
    f.down(Key::K);
    f.up(Key::K);
    let views = drain(&sub);
    assert!(views.iter().any(|v| matches!(&v.event, Event::Invalid { message } if message.contains("Grab Text"))));
    f.store.unblock();
    f.settle(1);
    assert_eq!(catalog_of(&f).revision, 1);
}

/// A failed write leaves the binding as it was, tells the recording and the catalog, and rebases what is queued.
#[test]
fn a_failed_write_changes_nothing() {
    let f = fake();
    f.store.fail.store(true, std::sync::atomic::Ordering::Release);
    let owner = f.input.owner();
    let rec = owner.record("grab").unwrap();
    let sub = rec.observe();
    f.mod_down(Mods::CTRL_L);
    f.down(Key::K);
    f.up(Key::K);
    f.settle(1);
    assert!(drain(&sub).iter().any(|v| matches!(&v.event, Event::Invalid { message } if message == "disk full")));
    let view = catalog_of(&f);
    assert_eq!(view.revision, 0);
    assert!(view.last_error.is_some());
    assert!(f.store.get("grab").is_none());
}

/// Reset writes the default back and answers with the new revision once it is saved.
#[test]
fn reset_restores_the_default() {
    let f = fake();
    let owner = f.input.owner();
    let _rec = owner.record("grab").unwrap();
    f.mod_down(Mods::CTRL_L);
    f.down(Key::K);
    f.up(Key::K);
    f.settle(1);
    let done = f.input.reset("grab");
    f.settle(2);
    assert!(matches!(done.wait(Duration::from_secs(2)), Some(Ok(2))));
    assert_eq!(f.store.get("grab").unwrap()["modifiers"], "CTRL_LEFT | OPT_LEFT");
    assert!(matches!(f.input.reset("nope").try_get(), Some(Err(InputError::UnknownShortcut(_)))));
}

/// The ninth unfinished write is refused as busy.
#[test]
fn the_write_queue_is_bounded() {
    let f = fake();
    f.store.block();
    let answers: Vec<_> = (0..10).map(|_| f.input.reset("grab")).collect();
    let busy = answers.iter().filter(|a| matches!(a.try_get(), Some(Err(InputError::Busy)))).count();
    assert!(busy >= 1, "some reset was refused as busy");
    f.store.unblock();
}

/// Stored rows that clash or cannot be read stay inactive with a visible message and are not overwritten.
#[test]
fn invalid_stored_rows_stay_inactive() {
    let store = FakeStore::new();
    store.put("grab", serde_json::json!({"modifiers": "CTRL_LEFT", "key": "Space"}));
    store.put("selection", serde_json::json!({"modifiers": "CTRL_LEFT", "key": "NoSuchKey"}));
    let f = Fake::with_store(&CAT, store.clone());
    let view = catalog_of(&f);
    let grab = view.shortcuts.iter().find(|s| s.id == "grab").unwrap();
    let selection = view.shortcuts.iter().find(|s| s.id == "selection").unwrap();
    assert!(grab.error.as_deref().unwrap().contains("Dictation"));
    assert!(selection.error.is_some());
    assert_eq!(store.get("grab").unwrap(), serde_json::json!({"modifiers": "CTRL_LEFT", "key": "Space"}), "never rewritten");
    f.mod_down(Mods::CTRL_L);
    assert_eq!(f.down(Key::Space), crate::input::core::Verdict::Swallow);
    assert_eq!(f.active(), vec![0], "only dictation fires");
}

/// A fixture of the current stored shortcuts loads and shows without rebinding.
#[test]
fn current_stored_shortcuts_load_unchanged() {
    let store = FakeStore::new();
    store.put("dictation", serde_json::json!({"modifiers": "CTRL_LEFT", "key": "Space"}));
    store.put("grab", serde_json::json!({"modifiers": "CTRL_LEFT | OPT_LEFT", "key": "G"}));
    let f = Fake::with_store(&CAT, store.clone());
    let view = catalog_of(&f);
    assert!(view.shortcuts.iter().all(|s| s.error.is_none()));
    assert_eq!(view.shortcuts[0].parts.iter().map(|p| p.label.as_str()).collect::<Vec<_>>(), vec!["Ctrl", "Space"]);
    assert_eq!(store.writes_len(), 0);
}
