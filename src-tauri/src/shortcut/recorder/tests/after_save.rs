//! File: shortcut/recorder/tests/after_save.rs
//! Purpose: Replays key streams that go on after a combination was saved: the recording stays open and the next combination replaces it.
//! Contents: the tests, using the rig of the parent module.

use super::*;

#[test]
fn a_second_combination_replaces_the_first_and_the_recording_stays_open() {
    let rig = Rig::new();
    rig.record(
        DICTATION,
        vec![
            modifier(CTRL, true),
            key(CTRL, Key::K, true),
            key(CTRL, Key::K, false),
            modifier(NONE, false),
            modifier(ALT, true),
            key(ALT, Key::J, true),
            key(ALT, Key::J, false),
            modifier(NONE, false),
        ],
    );
    let second = Hotkey::new(ALT, Key::J).unwrap();
    assert_eq!(rig.bound(DICTATION), second);
    assert_eq!(rig.saves.borrow().len(), 2);
    assert_eq!(rig.last_saved().unwrap(), ["alt", "j"]);
    assert_eq!(rig.only_done().outcome, Outcome::Cancelled { reason: Reason::Expired });
}

#[test]
fn escape_after_a_save_closes_and_keeps_the_saved_combination() {
    let rig = Rig::new();
    rig.record(
        DICTATION,
        vec![modifier(CTRL, true), key(CTRL, Key::K, true), key(CTRL, Key::K, false), modifier(NONE, false), key(NONE, Key::Escape, true)],
    );
    assert_eq!(rig.only_done().outcome, Outcome::Cancelled { reason: Reason::User });
    assert_eq!(rig.bound(DICTATION), Hotkey::new(CTRL, Key::K).unwrap());
}

#[test]
fn a_failed_store_write_puts_the_shortcut_back_and_tells_the_page() {
    let rig = Rig::new();
    let before = rig.bound(DICTATION);
    rig.fail_saves.set(true);
    rig.record(DICTATION, vec![modifier(CTRL, true), key(CTRL, Key::K, true), key(CTRL, Key::K, false), modifier(NONE, false)]);
    assert_eq!(rig.bound(DICTATION), before);
    assert_eq!(rig.invalids(), ["Could not save the shortcut: disk full."]);
    assert!(rig.last_saved().is_none());
}

#[test]
fn a_cancel_that_arrives_before_the_start_stops_the_start() {
    let rig = Rig::new();
    rig.with(|host| {
        assert!(!cancel_request(host, DICTATION, "r1"));
        assert_eq!(begin(host, DICTATION, "r1"), None);
        assert!(begin(host, DICTATION, "r1").is_some(), "the id is spent once it has stopped one start");
    });
}

#[test]
fn a_cancel_by_request_ends_only_that_recording() {
    let rig = Rig::new();
    rig.with(|host| {
        begin(host, DICTATION, "mine").unwrap();
        assert!(!cancel_request(host, DICTATION, "someone-else"));
        assert!(cancel_request(host, DICTATION, "mine"));
    });
    assert_eq!(rig.only_done().outcome, Outcome::Cancelled { reason: Reason::Outside });
}

#[test]
fn a_cancel_while_the_store_write_is_pending_cannot_leave_the_new_binding_half_applied() {
    let rig = Rig::new();
    let before = rig.bound(DICTATION);
    rig.fail_saves.set(true);
    rig.record(DICTATION, vec![modifier(CTRL, true), key(CTRL, Key::K, true), key(CTRL, Key::K, false), modifier(NONE, false)]);
    assert_eq!(rig.bound(DICTATION), before);
    assert!(rig.engine.lock().recording.is_none());
    assert!(rig.engine.blocking.lock().unwrap().iter().all(|h| *h != Hotkey::new(CTRL, Key::K).unwrap()));
}
