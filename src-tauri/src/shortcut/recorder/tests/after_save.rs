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
