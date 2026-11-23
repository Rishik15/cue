//! File: platform/tests/typing_keys.rs
//! Purpose: Protect the rule that a modifier chord is never a typed delimiter or Backspace, so a snippet is not fired or text erased by Ctrl+Space, Alt+Enter, Win+Space or Shift+Tab.
//! Contents: tests of `is_chord` and `classify`.

use crate::platform::typing_keys::{classify, is_chord, Held, Special};
use crate::platform::Typed;

const KEYS: [Special; 4] = [Special::Back, Special::Return, Special::Tab, Special::Space];

/// Builds the held set from the four flags.
fn held(shift: bool, ctrl: bool, alt: bool, win: bool) -> Held {
    Held { shift, ctrl, alt, win }
}

/// Ctrl, Alt, Win or Ctrl+Shift with any of the four keys resets.
#[test]
fn chords_reset() {
    for key in KEYS {
        for h in [
            held(false, true, false, false),
            held(false, false, true, false),
            held(false, false, false, true),
            held(true, true, false, false),
        ] {
            assert_eq!(classify(key, h), Typed::Reset, "{key:?} {h:?}");
        }
    }
}

/// Shift+Tab resets; Shift with Space or Return still types; AltGr types.
#[test]
fn shift_and_altgr() {
    assert_eq!(classify(Special::Tab, held(true, false, false, false)), Typed::Reset);
    assert_eq!(classify(Special::Space, held(true, false, false, false)), Typed::Char(' '));
    assert_eq!(classify(Special::Return, held(true, false, false, false)), Typed::Char('\n'));
    assert_eq!(classify(Special::Space, held(false, true, true, false)), Typed::Char(' '));
    assert_eq!(classify(Special::Back, held(false, true, true, false)), Typed::Backspace);
}

/// Bare keys keep their meaning.
#[test]
fn bare_keys() {
    let none = Held::default();
    assert_eq!(classify(Special::Return, none), Typed::Char('\n'));
    assert_eq!(classify(Special::Tab, none), Typed::Char('\t'));
    assert_eq!(classify(Special::Space, none), Typed::Char(' '));
    assert_eq!(classify(Special::Back, none), Typed::Backspace);
}

/// All 16 modifier combinations agree with `win || ctrl != alt`.
#[test]
fn chord_table() {
    for bits in 0..16u8 {
        let (shift, ctrl, alt, win) = (bits & 1 != 0, bits & 2 != 0, bits & 4 != 0, bits & 8 != 0);
        assert_eq!(is_chord(held(shift, ctrl, alt, win)), win || ctrl != alt, "{bits:04b}");
    }
}
