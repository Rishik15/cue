//! File: shortcut/recorder/tests/ending.rs
//! Purpose: Replays the ways a recording is replaced, cancelled or reset, and checks each ends with one final event.
//! Contents: the tests, using the rig of the parent module.

use super::*;

#[test]
fn starting_another_recording_replaces_the_first_with_one_cancelled_event() {
    let rig = Rig::new();
    let (first, second) = rig.with(|host| {
        let first = begin(host, DICTATION, "").unwrap();
        (first, begin(host, GRAB, "").unwrap())
    });
    let replaced = rig.only_done();
    assert_eq!((replaced.name, replaced.serial), (DICTATION, first));
    assert_eq!(replaced.outcome, Outcome::Cancelled { reason: Reason::Replaced });
    let running = {
        let s = rig.engine.lock();
        (s.recording, s.serial)
    };
    assert_eq!(running, (Some(GRAB), second));
    rig.with(|host| run(host, &script(vec![key(NONE, Key::Escape, true)]), DICTATION, first, &QUICK));
    assert_eq!(rig.dones().len(), 1, "the replaced recording's thread adds nothing");
}

#[test]
fn closing_the_window_cancels_the_running_recording() {
    let rig = Rig::new();
    rig.with(|host| {
        begin(host, DICTATION, "").unwrap();
        crate::shortcut::session::cancel_any(host, Reason::WindowClosed);
    });
    assert_eq!(rig.only_done().outcome, Outcome::Cancelled { reason: Reason::WindowClosed });
}

/// A reset stops the recording under its own lock, so a combination finished a moment later is refused and the reset stays.
#[test]
fn a_reset_stops_the_recording_before_a_late_commit_can_overwrite_it() {
    let rig = Rig::new();
    let serial = rig.with(|host| begin(host, DICTATION, "").unwrap());
    let running = crate::shortcut::session::take_running(&mut rig.engine.lock());
    let late = Hotkey::new(CTRL | ALT, Key::K).expect("a valid combination");
    let verdict = rig.with(|host| commit(host, DICTATION, serial, late));
    assert_eq!(verdict, Verdict::Gone);
    rig.with(|host| crate::shortcut::session::announce(host, running, Outcome::Cancelled { reason: Reason::Outside }));
    assert_eq!(rig.only_done().outcome, Outcome::Cancelled { reason: Reason::Outside });
    assert!(rig.saves.borrow().is_empty());
}
