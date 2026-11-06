//! File: shortcut/recorder/tests.rs
//! Purpose: Replays scripted key streams through a whole recording (source, gesture, session, saved bindings) with no window and no Windows hook.
//! Contents: Script — the in-memory key source; Rig — an engine plus the events and saves a recording produced; the tests (those about what happens after a save are in `after_save`).

use std::cell::RefCell;
use std::collections::{HashSet, VecDeque};
use std::sync::{Arc, Mutex};

use handy_keys::{Hotkey, Key, KeyEvent, Modifiers};

use super::*;
use crate::shortcut::events::{Done, Event, Outcome, Reason};
use crate::shortcut::registry::SHORTCUTS;
use crate::shortcut::session::{begin, commit, end, Host, Verdict};
use crate::shortcut::{Binding, Shared, DICTATION, GRAB};

const CTRL: Modifiers = Modifiers::CTRL_LEFT;
const ALT: Modifiers = Modifiers::OPT_LEFT;
const SHIFT: Modifiers = Modifiers::SHIFT_LEFT;
const WIN: Modifiers = Modifiers::CMD_LEFT;
const NONE: Modifiers = Modifiers::empty();
const QUICK: Limits = Limits { look: Duration::from_millis(1), longest: Duration::from_millis(30) };

/// A key source that plays a fixed list and then stays idle.
struct Script(RefCell<VecDeque<Pull>>);

impl KeySource for Script {
    /// Plays the next scripted pull, or idles when the list is spent.
    fn pull(&self, _wait: Duration) -> Pull {
        self.0.borrow_mut().pop_front().unwrap_or(Pull::Idle)
    }
}

/// A key source that plays `events` and then idles.
fn script(events: Vec<Pull>) -> Script {
    Script(RefCell::new(events.into()))
}

/// A scripted event: a modifier-only change to `modifiers`.
fn modifier(modifiers: Modifiers, down: bool) -> Pull {
    Pull::Event(KeyEvent { modifiers, key: None, is_key_down: down, changed_modifier: None })
}

/// A scripted event: `key` going down or up with `modifiers` held.
fn key(modifiers: Modifiers, key: Key, down: bool) -> Pull {
    Pull::Event(KeyEvent { modifiers, key: Some(key), is_key_down: down, changed_modifier: None })
}

/// An engine with the default shortcuts, and everything a recording sent out.
struct Rig {
    engine: Engine,
    events: RefCell<Vec<Event>>,
    saves: RefCell<Vec<(&'static str, Hotkey)>>,
}

impl Rig {
    /// An engine with the default shortcuts and nothing recorded.
    fn new() -> Rig {
        let bindings = std::array::from_fn(|i| {
            let name = SHORTCUTS[i].name;
            Binding { name, hotkey: crate::shortcut::default_hotkey(name), enabled: true, shortcut_on: true }
        });
        let shared = Shared { bindings, recording: None, serial: 0 };
        Rig {
            engine: Engine { shared: Mutex::new(shared), blocking: Arc::new(Mutex::new(HashSet::new())) },
            events: RefCell::new(Vec::new()),
            saves: RefCell::new(Vec::new()),
        }
    }

    /// Runs `f` with a host that records events and saves in this rig.
    fn with<R>(&self, f: impl FnOnce(&Host) -> R) -> R {
        let emit = |e: Event| self.events.borrow_mut().push(e);
        let persist = |name: &'static str, h: &Hotkey| self.saves.borrow_mut().push((name, *h));
        f(&Host { engine: &self.engine, emit: &emit, persist: &persist })
    }

    /// Starts a recording of `name` and plays `pulls` into it; returns its serial.
    fn record(&self, name: &'static str, pulls: Vec<Pull>) -> u64 {
        self.with(|host| {
            let serial = begin(host, name);
            run(host, &script(pulls), name, serial, &QUICK);
            serial
        })
    }

    /// The combination of the last `saved` event, if any.
    fn last_saved(&self) -> Option<Vec<String>> {
        self.events.borrow().iter().rev().find_map(|e| {
            if let Event::Saved(v) = e {
                Some(v.parts.iter().map(|p| p.code.clone()).collect())
            } else {
                None
            }
        })
    }

    /// Every final event the rig saw, in order.
    fn dones(&self) -> Vec<Done> {
        self.events.borrow().iter().filter_map(|e| if let Event::Done(d) = e { Some(d.clone()) } else { None }).collect()
    }

    /// The messages of every refused combination, in order.
    fn invalids(&self) -> Vec<String> {
        self.events.borrow().iter().filter_map(|e| if let Event::Invalid(i) = e { Some(i.message.clone()) } else { None }).collect()
    }

    /// The combination `name` is bound to now.
    fn bound(&self, name: &str) -> Hotkey {
        self.engine.lock().bindings.iter().find(|b| b.name == name).map(|b| b.hotkey).unwrap()
    }

    /// The only final event the rig saw; fails when there is none or more than one.
    fn only_done(&self) -> Done {
        let dones = self.dones();
        assert_eq!(dones.len(), 1, "exactly one final event, got {dones:?}");
        dones[0].clone()
    }
}

/// Asserts that `name` is bound to `expected`, that it was stored once and announced, that the recording stayed open until it expired, and that nothing is left running.
fn saved(rig: &Rig, name: &str, expected: Hotkey) {
    assert_eq!(rig.only_done().outcome, Outcome::Cancelled { reason: Reason::Expired });
    assert_eq!(rig.bound(name), expected);
    assert_eq!(rig.saves.borrow().as_slice(), [(DICTATION, expected)]);
    assert!(rig.last_saved().is_some());
    assert!(rig.engine.lock().recording.is_none());
}

#[test]
fn ctrl_k_is_saved_with_one_final_event() {
    let rig = Rig::new();
    rig.record(DICTATION, vec![modifier(CTRL, true), key(CTRL, Key::K, true), key(CTRL, Key::K, false), modifier(NONE, false)]);
    saved(&rig, DICTATION, Hotkey::new(CTRL, Key::K).unwrap());
    assert_eq!(rig.last_saved().unwrap(), ["ctrl", "k"]);
    assert_eq!(rig.only_done().parts.iter().map(|p| p.code.as_str()).collect::<Vec<_>>(), ["ctrl", "k"]);
}

#[test]
fn ctrl_alt_k_is_saved() {
    let rig = Rig::new();
    let both = CTRL | ALT;
    rig.record(DICTATION, vec![modifier(CTRL, true), modifier(both, true), key(both, Key::K, true), key(both, Key::K, false)]);
    saved(&rig, DICTATION, Hotkey::new(both, Key::K).unwrap());
}

#[test]
fn k_then_ctrl_released_first_is_still_saved() {
    let rig = Rig::new();
    rig.record(DICTATION, vec![modifier(CTRL, true), key(CTRL, Key::K, true), modifier(NONE, false), key(NONE, Key::K, false)]);
    saved(&rig, DICTATION, Hotkey::new(CTRL, Key::K).unwrap());
}

#[test]
fn win_k_is_saved() {
    let rig = Rig::new();
    rig.record(DICTATION, vec![modifier(WIN, true), key(WIN, Key::K, true), key(WIN, Key::K, false), modifier(NONE, false)]);
    saved(&rig, DICTATION, Hotkey::new(WIN, Key::K).unwrap());
}

#[test]
fn modifier_only_ctrl_shift_is_saved_when_all_keys_are_up() {
    let rig = Rig::new();
    let both = CTRL | SHIFT;
    rig.record(DICTATION, vec![modifier(CTRL, true), modifier(both, true), modifier(CTRL, false), modifier(NONE, false)]);
    saved(&rig, DICTATION, Hotkey::new(both, None).unwrap());
}

#[test]
fn bare_escape_cancels_and_keeps_the_old_shortcut() {
    let rig = Rig::new();
    let before = rig.bound(DICTATION);
    rig.record(DICTATION, vec![key(NONE, Key::Escape, true)]);
    assert_eq!(rig.only_done().outcome, Outcome::Cancelled { reason: Reason::User });
    assert_eq!(rig.bound(DICTATION), before);
    assert!(rig.saves.borrow().is_empty());
}

#[test]
fn ctrl_escape_is_a_normal_chord() {
    let rig = Rig::new();
    rig.record(DICTATION, vec![modifier(CTRL, true), key(CTRL, Key::Escape, true), key(CTRL, Key::Escape, false), modifier(NONE, false)]);
    saved(&rig, DICTATION, Hotkey::new(CTRL, Key::Escape).unwrap());
}

#[test]
fn a_lone_ctrl_keeps_the_recording_open_until_it_expires() {
    let rig = Rig::new();
    rig.record(DICTATION, vec![modifier(CTRL, true), modifier(NONE, false)]);
    assert_eq!(rig.only_done().outcome, Outcome::Cancelled { reason: Reason::Expired });
    assert!(rig.invalids().is_empty());
    assert!(rig.saves.borrow().is_empty());
}

#[test]
fn a_refused_combination_stays_open_for_another_try() {
    let rig = Rig::new();
    rig.record(
        GRAB,
        vec![
            key(NONE, Key::K, true),
            key(NONE, Key::K, false),
            modifier(CTRL, true),
            key(CTRL, Key::Space, true),
            key(CTRL, Key::Space, false),
            modifier(NONE, false),
            modifier(CTRL, true),
            key(CTRL, Key::K, true),
            key(CTRL, Key::K, false),
        ],
    );
    let refused = rig.invalids();
    assert_eq!(refused.len(), 2);
    assert!(refused[0].starts_with("Add a modifier key"));
    assert_eq!(refused[1], "Already used by Dictation.");
    assert_eq!(rig.only_done().outcome, Outcome::Cancelled { reason: Reason::Expired });
    assert_eq!(rig.bound(GRAB), Hotkey::new(CTRL, Key::K).unwrap());
}

#[test]
fn an_unfinished_recording_expires_with_one_final_event() {
    let rig = Rig::new();
    rig.record(DICTATION, vec![]);
    assert_eq!(rig.only_done().outcome, Outcome::Cancelled { reason: Reason::Expired });
    assert!(rig.engine.lock().recording.is_none());
}

#[test]
fn a_source_that_stops_fails_the_recording_once() {
    let rig = Rig::new();
    rig.record(DICTATION, vec![Pull::Closed]);
    assert!(matches!(rig.only_done().outcome, Outcome::Failed { .. }));
    assert!(rig.engine.lock().recording.is_none());
}

#[test]
fn a_start_failure_ends_with_one_failed_event() {
    let rig = Rig::new();
    rig.with(|host| {
        let serial = begin(host, DICTATION);
        assert!(end(host, DICTATION, serial, Outcome::Failed { message: "no hook".into() }));
        assert!(!end(host, DICTATION, serial, Outcome::Cancelled { reason: Reason::Outside }));
    });
    assert_eq!(rig.only_done().outcome, Outcome::Failed { message: "no hook".into() });
    assert!(rig.engine.lock().recording.is_none());
}

#[test]
fn a_stale_serial_or_another_control_cannot_end_or_save_a_recording() {
    let rig = Rig::new();
    rig.with(|host| {
        let serial = begin(host, DICTATION);
        assert!(!end(host, DICTATION, serial + 1, Outcome::Cancelled { reason: Reason::Outside }));
        assert!(!end(host, GRAB, serial, Outcome::Cancelled { reason: Reason::Outside }));
        assert_eq!(commit(host, DICTATION, serial + 1, Hotkey::new(CTRL, Key::K).unwrap()), Verdict::Gone);
        assert!(current(host.engine, DICTATION, serial));
    });
    assert!(rig.dones().is_empty());
    assert!(rig.saves.borrow().is_empty());
}

#[test]
fn starting_another_recording_replaces_the_first_with_one_cancelled_event() {
    let rig = Rig::new();
    let (first, second) = rig.with(|host| {
        let first = begin(host, DICTATION);
        (first, begin(host, GRAB))
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
        begin(host, DICTATION);
        crate::shortcut::session::cancel_any(host, Reason::WindowClosed);
    });
    assert_eq!(rig.only_done().outcome, Outcome::Cancelled { reason: Reason::WindowClosed });
}

/// A reset stops the recording under its own lock, so a combination finished a moment later is refused and the reset stays.
#[test]
fn a_reset_stops_the_recording_before_a_late_commit_can_overwrite_it() {
    let rig = Rig::new();
    let serial = rig.with(|host| begin(host, DICTATION));
    let running = crate::shortcut::session::take_running(&mut rig.engine.lock());
    let late = Hotkey::new(CTRL | ALT, Key::K).expect("a valid combination");
    let verdict = rig.with(|host| commit(host, DICTATION, serial, late));
    assert_eq!(verdict, Verdict::Gone);
    rig.with(|host| crate::shortcut::session::announce(host, running, Outcome::Cancelled { reason: Reason::Outside }));
    assert_eq!(rig.only_done().outcome, Outcome::Cancelled { reason: Reason::Outside });
    assert!(rig.saves.borrow().is_empty());
}

mod after_save;
