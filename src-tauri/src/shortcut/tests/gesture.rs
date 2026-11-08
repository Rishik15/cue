//! File: shortcut/tests/gesture.rs
//! Purpose: Protect recording a shortcut: replays the key events a person produces (a chord, a modifier-only combination, a stray tap, Escape, key repeat, a second key) and checks when the combination is complete.
//! Contents: scenario tests of `Gesture::feed`.

use handy_keys::{Hotkey, Key, KeyEvent, Modifiers};

use crate::shortcut::gesture::{Gesture, Step};

const CTRL: Modifiers = Modifiers::CTRL_LEFT;
const ALT: Modifiers = Modifiers::OPT_LEFT;

/// A main key event. Inputs: the key, the modifiers held and whether it went down. Returns the event the hook would send.
fn key(key: Key, modifiers: Modifiers, down: bool) -> KeyEvent {
    KeyEvent { modifiers, key: Some(key), is_key_down: down, changed_modifier: None }
}

/// A modifier-only event. Inputs: the modifiers held after it and whether one went down. Returns the event the hook would send.
fn modifier(modifiers: Modifiers, down: bool) -> KeyEvent {
    KeyEvent { modifiers, key: None, is_key_down: down, changed_modifier: Some(modifiers) }
}

/// The hotkey a combination makes. Inputs: modifiers and key. Returns it.
fn chord(modifiers: Modifiers, key: impl Into<Option<Key>>) -> Hotkey {
    Hotkey::new(modifiers, key).expect("chord")
}

/// Whether a step shows keys. Input: the step. Returns the `keyed` flag, None if it shows nothing.
fn live(step: &Step) -> Option<bool> {
    match step {
        Step::Live { keyed, .. } => Some(*keyed),
        _ => None,
    }
}

/// Ctrl+Space is complete when Space is released, even if Ctrl went up first; a held key's repeats redraw nothing.
#[test]
fn a_chord_completes_when_its_main_key_is_released() {
    let mut gesture = Gesture::default();
    assert_eq!(live(&gesture.feed(&modifier(CTRL, true))), Some(false));
    assert_eq!(live(&gesture.feed(&key(Key::Space, CTRL, true))), Some(true));
    assert_eq!(gesture.feed(&key(Key::Space, CTRL, true)), Step::Ignore, "key repeat");
    assert_eq!(gesture.feed(&modifier(Modifiers::empty(), false)), Step::Ignore, "modifiers up first");
    assert_eq!(gesture.feed(&key(Key::Space, Modifiers::empty(), false)), Step::Commit(chord(CTRL, Key::Space)));
}

/// The first chord pressed is the one kept: a second key pressed meanwhile does not replace it, and its release changes nothing.
#[test]
fn the_first_chord_is_kept_when_another_key_joins() {
    let mut gesture = Gesture::default();
    gesture.feed(&key(Key::A, CTRL, true));
    assert_eq!(gesture.feed(&key(Key::B, CTRL, true)), Step::Ignore);
    assert_eq!(gesture.feed(&key(Key::B, CTRL, false)), Step::Ignore);
    assert_eq!(gesture.feed(&key(Key::A, CTRL, false)), Step::Commit(chord(CTRL, Key::A)));
}

/// Ctrl then Alt held and let go (in any order) is a modifier-only shortcut, complete when the last one is up.
#[test]
fn modifiers_alone_complete_when_all_are_up() {
    let mut gesture = Gesture::default();
    assert_eq!(live(&gesture.feed(&modifier(CTRL, true))), Some(false));
    assert_eq!(live(&gesture.feed(&modifier(CTRL | ALT, true))), Some(false));
    assert_eq!(gesture.feed(&modifier(ALT, false)), Step::Ignore, "one still held");
    assert_eq!(gesture.feed(&modifier(Modifiers::empty(), false)), Step::Commit(chord(CTRL | ALT, None)));
}

/// A lone Ctrl tap is dropped and the recording stays open for another try.
#[test]
fn a_lone_ctrl_tap_is_dropped() {
    let mut gesture = Gesture::default();
    gesture.feed(&modifier(CTRL, true));
    assert_eq!(gesture.feed(&modifier(Modifiers::empty(), false)), Step::Ignore);
    assert_eq!(live(&gesture.feed(&modifier(CTRL, true))), Some(false), "shown again, not mistaken for a repeat");
}

/// A bare Escape cancels; Escape with a modifier is an ordinary chord, and Escape after a chord started does not cancel.
#[test]
fn only_a_bare_escape_cancels() {
    assert_eq!(Gesture::default().feed(&key(Key::Escape, Modifiers::empty(), true)), Step::Cancel);
    assert_eq!(live(&Gesture::default().feed(&key(Key::Escape, CTRL, true))), Some(true));
    let mut gesture = Gesture::default();
    gesture.feed(&key(Key::K, CTRL, true));
    assert_ne!(gesture.feed(&key(Key::Escape, Modifiers::empty(), true)), Step::Cancel);
}

/// After a refused combination, reset starts a fresh try with nothing remembered.
#[test]
fn reset_forgets_the_combination_so_far() {
    let mut gesture = Gesture::default();
    gesture.feed(&key(Key::A, CTRL, true));
    gesture.reset();
    assert_eq!(gesture.feed(&key(Key::A, CTRL, false)), Step::Ignore);
    assert_eq!(live(&gesture.feed(&key(Key::B, ALT, true))), Some(true));
    assert_eq!(gesture.feed(&key(Key::B, ALT, false)), Step::Commit(chord(ALT, Key::B)));
}
