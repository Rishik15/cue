//! File: input/tests/gesture.rs
//! Purpose: Protect when a recording decides a combination is complete: held modifiers then a key, either release order, repeated key downs, a second key while one is held, reset, and Escape.
//! Contents: scenario tests that feed `Ev` values straight to `Gesture`.

use crate::input::gesture::{Ev, Gesture, Step};
use crate::input::{Chord, Key, Mods};

const CTRL: Mods = Mods::CTRL_L;
const BOTH: Mods = Mods(Mods::CTRL_L.0 | Mods::SHIFT_L.0);

/// A key event. Inputs: the key, whether it went down and the modifiers held. Returns the event.
fn key(k: Key, down: bool, held: Mods) -> Ev {
    Ev { key: Some(k), down, held }
}

/// A modifier-only event. Inputs: whether it went down and the modifiers held after it. Returns the event.
fn modifier(down: bool, held: Mods) -> Ev {
    Ev { key: None, down, held }
}

/// A chord that must be valid. Inputs: the modifiers and the key. Returns the chord.
fn chord(m: Mods, k: Option<Key>) -> Chord {
    Chord::new(m, k).unwrap()
}

/// Modifiers down, then the key: every event shows the keys, and the key's release completes the combination.
#[test]
fn held_modifiers_then_key_commits_on_key_release() {
    let mut g = Gesture::default();
    assert!(matches!(g.feed(modifier(true, CTRL)), Step::Live { keyed: false, .. }));
    assert!(matches!(g.feed(key(Key::K, true, CTRL)), Step::Live { keyed: true, .. }));
    assert_eq!(g.feed(key(Key::K, false, CTRL)), Step::Commit(chord(CTRL, Some(Key::K))));
}

/// Modifiers released before the key do not complete or change it; the key's release does.
#[test]
fn modifiers_released_first_still_commit_on_key_release() {
    let mut g = Gesture::default();
    g.feed(modifier(true, CTRL));
    g.feed(key(Key::K, true, CTRL));
    assert_eq!(g.feed(modifier(false, Mods::NONE)), Step::Ignore);
    assert_eq!(g.feed(key(Key::K, false, Mods::NONE)), Step::Commit(chord(CTRL, Some(Key::K))));
}

/// A repeated key down shows nothing new; after the engine resets the gesture on a commit, a stray second release completes nothing.
#[test]
fn repeated_key_down_is_ignored_and_a_reset_commit_is_not_repeated() {
    let mut g = Gesture::default();
    g.feed(modifier(true, CTRL));
    assert!(matches!(g.feed(key(Key::K, true, CTRL)), Step::Live { .. }));
    assert_eq!(g.feed(key(Key::K, true, CTRL)), Step::Ignore);
    assert_eq!(g.feed(key(Key::K, false, CTRL)), Step::Commit(chord(CTRL, Some(Key::K))));
    g.reset();
    assert_eq!(g.feed(key(Key::K, false, CTRL)), Step::Ignore);
}

/// A second key pressed while the first is held keeps the first combination, and only the first key's release completes it.
#[test]
fn the_first_key_wins() {
    let mut g = Gesture::default();
    g.feed(modifier(true, CTRL));
    g.feed(key(Key::K, true, CTRL));
    assert_eq!(g.feed(key(Key::J, true, CTRL)), Step::Ignore);
    assert_eq!(g.feed(key(Key::J, false, CTRL)), Step::Ignore);
    assert_eq!(g.feed(key(Key::K, false, CTRL)), Step::Commit(chord(CTRL, Some(Key::K))));
}

/// A modifier-only combination completes when the last modifier is up.
#[test]
fn modifier_only_commits_when_all_are_up() {
    let mut g = Gesture::default();
    g.feed(modifier(true, CTRL));
    g.feed(modifier(true, BOTH));
    assert_eq!(g.feed(modifier(false, CTRL)), Step::Ignore);
    assert_eq!(g.feed(modifier(false, Mods::NONE)), Step::Commit(chord(BOTH, None)));
}

/// A tap too short to be usable clears the keycaps instead of completing.
#[test]
fn an_unusable_tap_clears() {
    let mut g = Gesture::default();
    g.feed(modifier(true, Mods::SHIFT_L));
    assert_eq!(g.feed(modifier(false, Mods::NONE)), Step::Clear);
}

/// Reset forgets the combination so the next try starts clean.
#[test]
fn reset_forgets_the_combination() {
    let mut g = Gesture::default();
    g.feed(modifier(true, CTRL));
    g.feed(key(Key::K, true, CTRL));
    g.reset();
    assert_eq!(g.feed(key(Key::K, false, CTRL)), Step::Ignore);
    g.feed(key(Key::J, true, CTRL));
    assert_eq!(g.feed(key(Key::J, false, CTRL)), Step::Commit(chord(CTRL, Some(Key::J))));
}

/// A bare Escape cancels even with a key held; Escape with a modifier is a combination.
#[test]
fn bare_escape_cancels() {
    let mut g = Gesture::default();
    g.feed(key(Key::K, true, Mods::NONE));
    assert_eq!(g.feed(key(Key::Escape, true, Mods::NONE)), Step::Cancel);
    let mut g = Gesture::default();
    assert!(matches!(g.feed(key(Key::Escape, true, CTRL)), Step::Live { keyed: true, .. }));
}
