//! File: input/tests/stored.rs
//! Purpose: Protect what settings.json already holds: modifiers are stored as the text the old library's bitflags wrote, and key names are the old variant names. The fixtures are real output of bitflags 2 with its serde feature, and the five default shortcuts as 0.3.5 wrote them.
//! Contents: tests of `Mods` text, key names, a full settings fixture, and rows with bits this build does not know.

use crate::input::fake::{Fake, FakeStore};
use crate::input::{Chord, Key, Mods, Verdict, CATALOG};

/// Pairs of bits and the exact text bitflags 2 wrote for them.
const LEGACY: [(u32, &str); 14] = [
    (0, ""),
    (1, "CMD_LEFT"),
    (2, "SHIFT_LEFT"),
    (4, "CTRL_LEFT"),
    (8, "OPT_LEFT"),
    (16, "FN"),
    (32, "CMD_RIGHT"),
    (64, "SHIFT_RIGHT"),
    (128, "CTRL_RIGHT"),
    (256, "OPT_RIGHT"),
    (132, "CTRL_LEFT | CTRL_RIGHT"),
    (12, "CTRL_LEFT | OPT_LEFT"),
    (511, "CMD_LEFT | SHIFT_LEFT | CTRL_LEFT | OPT_LEFT | FN | CMD_RIGHT | SHIFT_RIGHT | CTRL_RIGHT | OPT_RIGHT"),
    (516, "CTRL_LEFT | 0x200"),
];

/// Every legacy text loads to its bits and writes back the same text.
#[test]
fn modifier_text_is_what_bitflags_wrote() {
    for (bits, text) in LEGACY {
        let written = serde_json::to_string(&Mods(bits)).unwrap();
        assert_eq!(written, format!("\"{text}\""), "bits {bits}");
        assert_eq!(serde_json::from_str::<Mods>(&written).unwrap(), Mods(bits));
    }
}

/// The compound names bitflags also reads, a plain integer and an empty text load; an unknown name does not.
#[test]
fn compound_names_integers_and_unknown_names() {
    for (text, bits) in [("CTRL", 132), ("CMD", 33), ("OPT", 264), ("SHIFT", 66), ("CTRL | OPT_LEFT", 140), ("", 0), (" CTRL_LEFT ", 4)] {
        assert_eq!(serde_json::from_value::<Mods>(serde_json::json!(text)).unwrap(), Mods(bits), "{text:?}");
    }
    assert_eq!(serde_json::from_value::<Mods>(serde_json::json!(12)).unwrap(), Mods(12));
    for bad in ["BOGUS", "ctrl_left", "CTRL_LEFT | ", "CTRL_LEFT | NOPE"] {
        assert!(serde_json::from_value::<Mods>(serde_json::json!(bad)).is_err(), "{bad:?}");
    }
    assert!(serde_json::from_value::<Mods>(serde_json::json!(true)).is_err());
}

/// The main keys of the old library, by variant name; the ones Cue never offered again (mouse buttons, JIS keys, KeypadEquals, KeypadComma, Dictation) stay unknown so their rows are inactive with a message.
#[test]
fn key_names_match_the_old_variants() {
    let old = "A B C D E F G H I J K L M N O P Q R S T U V W X Y Z Num0 Num1 Num2 Num3 Num4 Num5 Num6 Num7 Num8 Num9 PlayPause Stop PrevTrack NextTrack F1 F2 F3 F4 F5 F6 F7 F8 F9 F10 F11 F12 F13 F14 F15 F16 F17 F18 F19 F20 F21 F22 F23 F24 Space Return Tab Escape Delete ForwardDelete Insert Pause Home End PageUp PageDown LeftArrow RightArrow UpArrow DownArrow Minus Equal LeftBracket RightBracket Backslash Semicolon Quote Comma Period Slash Grave Section Keypad0 Keypad1 Keypad2 Keypad3 Keypad4 Keypad5 Keypad6 Keypad7 Keypad8 Keypad9 KeypadDecimal KeypadMultiply KeypadPlus KeypadClear KeypadDivide KeypadEnter KeypadMinus CapsLock ScrollLock NumLock PrintScreen ContextMenu";
    for name in old.split(' ') {
        let key: Key = serde_json::from_value(serde_json::json!(name)).unwrap_or_else(|_| panic!("{name} must load"));
        assert_eq!(serde_json::to_value(key).unwrap(), serde_json::json!(name));
    }
    for gone in [
        "JisYen",
        "JisUnderscore",
        "JisEisu",
        "JisKana",
        "KeypadEquals",
        "KeypadComma",
        "Dictation",
        "MouseLeft",
        "MouseRight",
        "MouseMiddle",
        "MouseX1",
        "MouseX2",
    ] {
        assert!(serde_json::from_value::<Key>(serde_json::json!(gone)).is_err(), "{gone}");
    }
}

/// The five default shortcuts as 0.3.5 wrote them load as valid rows, equal to the defaults, and write back to the same JSON.
#[test]
fn a_0_3_5_settings_file_loads_unchanged() {
    let fixture = [
        ("dictation", serde_json::json!({"modifiers": "CTRL_LEFT", "key": "Space"})),
        ("selection", serde_json::json!({"modifiers": "CTRL_LEFT | OPT_LEFT", "key": "Space"})),
        ("grab", serde_json::json!({"modifiers": "CTRL_LEFT | OPT_LEFT", "key": "G"})),
        ("define", serde_json::json!({"modifiers": "CTRL_LEFT | OPT_LEFT", "key": "D"})),
        ("notes", serde_json::json!({"modifiers": "CTRL_LEFT | OPT_LEFT", "key": "N"})),
    ];
    let store = FakeStore::new();
    for (id, value) in &fixture {
        store.put(id, value.clone());
    }
    let f = Fake::with_store(&CATALOG, store);
    let view = f.input.catalog();
    assert_eq!(view.shortcuts.len(), 5);
    assert!(view.shortcuts.iter().all(|s| s.error.is_none()), "no stored row may be refused");
    for ((id, value), spec) in fixture.iter().zip(CATALOG.specs) {
        assert_eq!(spec.id, *id);
        let chord: Chord = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(Chord::new(spec.default.0, Some(spec.default.1)).unwrap(), chord);
        assert_eq!(&serde_json::to_value(chord).unwrap(), value);
    }
}

/// A row with a modifier bit this build does not know stays inactive with a message, and its main key keeps typing normally.
#[test]
fn unknown_modifier_bits_leave_the_row_inactive() {
    let store = FakeStore::new();
    store.put("grab", serde_json::json!({"modifiers": 512, "key": "A"}));
    store.put("f8", serde_json::json!({"modifiers": "CTRL_LEFT | 0x400", "key": "F8"}));
    let f = Fake::with_store(&super::CAT, store);
    let view = f.input.catalog();
    assert!(view.shortcuts[2].error.is_some() && view.shortcuts[3].error.is_some());
    assert_eq!(f.down(Key::A), Verdict::Pass);
    assert!(f.active().is_empty());
}

/// A row naming an unknown modifier is inactive with a message and is never rewritten.
#[test]
fn an_unknown_modifier_name_is_inactive_and_kept() {
    let store = FakeStore::new();
    let bad = serde_json::json!({"modifiers": "HYPER", "key": "K"});
    store.put("grab", bad.clone());
    let f = Fake::with_store(&super::CAT, store.clone());
    assert!(f.input.catalog().shortcuts[2].error.is_some());
    assert_eq!(store.get("grab").unwrap(), bad);
}
