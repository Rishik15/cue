//! File: input/tests/chord.rs
//! Purpose: Protect the stored shape of a combination and what counts as a clash: the exact JSON settings.json holds today, left, right, either and both sides, and the rules for which combinations may be saved.
//! Contents: tests of `Chord` serialization, `matches`, `intersects`, `nests`, `usable`, `reserved`, `parts`.

use crate::input::{Chord, Key, Mods};

fn chord(m: Mods, k: Option<Key>) -> Chord {
    Chord::new(m, k).unwrap()
}

/// The shape written by the old handy-keys `Hotkey` loads unchanged and writes back the same.
#[test]
fn stored_shape_is_unchanged() {
    let old = serde_json::json!({"modifiers": "CTRL_LEFT | OPT_LEFT", "key": "Space"});
    let loaded: Chord = serde_json::from_value(old.clone()).unwrap();
    assert_eq!(loaded, chord(Mods::CTRL_L | Mods::ALT_L, Some(Key::Space)));
    assert_eq!(serde_json::to_value(loaded).unwrap(), old);
    let modifier_only: Chord = serde_json::from_value(serde_json::json!({"modifiers": "CMD_LEFT | CTRL_LEFT", "key": null})).unwrap();
    assert_eq!(modifier_only.key, None);
    assert!(serde_json::from_value::<Chord>(serde_json::json!({"modifiers": "CMD_LEFT", "key": "NoSuchKey"})).is_err());
}

/// A left-only combination needs the left key, either accepts both, and another family held ends a match.
#[test]
fn sided_matching_follows_the_library() {
    let left = chord(Mods::CTRL_L, Some(Key::K));
    assert!(left.matches(Mods::CTRL_L));
    assert!(!left.matches(Mods::CTRL_R));
    assert!(left.matches(Mods::CTRL_L | Mods::CTRL_R));
    let either = chord(Mods::CTRL_L | Mods::CTRL_R, Some(Key::K));
    assert!(either.matches(Mods::CTRL_R) && either.matches(Mods::CTRL_L));
    assert!(!either.matches(Mods::CTRL_L | Mods::SHIFT_L));
    assert!(!either.matches(Mods::NONE));
}

/// Left, right, either and both clash exactly when one held state satisfies both (the library lets a left-only shortcut match with the right key also down).
#[test]
fn collisions_use_real_intersection() {
    let l = chord(Mods::CTRL_L, Some(Key::K));
    let r = chord(Mods::CTRL_R, Some(Key::K));
    let either = chord(Mods::CTRL_L | Mods::CTRL_R, Some(Key::K));
    let mut both = either;
    both.both = Mods::CTRL_L | Mods::CTRL_R;
    assert!(l.intersects(&r), "with both Ctrl keys down both would match, so they clash");
    assert!(l.intersects(&either) && r.intersects(&either));
    assert!(l.intersects(&both) && r.intersects(&both) && either.intersects(&both));
    assert!(!l.intersects(&chord(Mods::CTRL_L, Some(Key::J))));
}

/// A modifier-only shortcut inside a longer one clashes; a bare letter and a lone Ctrl are refused; a bare Escape is reserved.
#[test]
fn what_may_be_saved() {
    assert!(chord(Mods::CTRL_L | Mods::SHIFT_L, None).nests(&chord(Mods::CTRL_L | Mods::SHIFT_L, Some(Key::A))));
    assert!(!chord(Mods::NONE, Some(Key::A)).usable());
    assert!(chord(Mods::NONE, Some(Key::F8)).usable());
    assert!(!chord(Mods::CTRL_L, None).usable());
    assert!(chord(Mods::ALT_L, None).usable());
    assert!(chord(Mods::CTRL_L | Mods::SHIFT_L, None).usable());
    assert!(chord(Mods::NONE, Some(Key::Escape)).reserved());
    assert!(!chord(Mods::CTRL_L, Some(Key::Escape)).reserved());
}

/// Parts mark a single side and show the Copilot key as one key.
#[test]
fn parts_show_sides_and_copilot() {
    let parts = chord(Mods::CTRL_L | Mods::ALT_L | Mods::ALT_R, Some(Key::G)).parts();
    let sides: Vec<_> = parts.iter().map(|p| (p.code.as_str(), p.side)).collect();
    assert_eq!(sides, vec![("ctrl", Some('L')), ("alt", None), ("g", None)]);
    let copilot = chord(Mods::WIN_L | Mods::SHIFT_L, Some(Key::F23)).parts();
    assert_eq!(copilot.len(), 1);
    assert_eq!(copilot[0].code, "copilot");
}
