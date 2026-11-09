//! File: input/tests/keymap.rs
//! Purpose: Protect the mapping from Windows key events to slots: sided modifiers, positional punctuation on US and UK layouts, keypad Enter, the AltGr phantom Ctrl.
//! Contents: tests of `keymap::slot` and `is_phantom_ctrl`.

use crate::input::keymap::{is_phantom_ctrl, slot};
use crate::input::ledger::Slot;
use crate::input::{Key, Mods};

/// Sided modifiers map to their own bit and the generic ones to the left key.
#[test]
fn modifiers_are_sided() {
    assert_eq!(slot(0xA3, 0x1D, true), Some(Slot::Mod(Mods::CTRL_R)));
    assert_eq!(slot(0x11, 0x1D, false), Some(Slot::Mod(Mods::CTRL_L)));
    assert_eq!(slot(0x5C, 0x5C, true), Some(Slot::Mod(Mods::WIN_R)));
}

/// The generic Alt and Ctrl follow the extended flag (the right-hand key is extended); the explicit sided keys ignore it.
#[test]
fn a_generic_alt_or_ctrl_follows_the_extended_flag() {
    assert_eq!(slot(0x12, 0x38, false), Some(Slot::Mod(Mods::ALT_L)));
    assert_eq!(slot(0x12, 0x38, true), Some(Slot::Mod(Mods::ALT_R)));
    assert_eq!(slot(0x11, 0x1D, true), Some(Slot::Mod(Mods::CTRL_R)));
    assert_eq!(slot(0xA4, 0x38, false), Some(Slot::Mod(Mods::ALT_L)));
    assert_eq!(slot(0xA4, 0x38, true), Some(Slot::Mod(Mods::ALT_L)));
    assert_eq!(slot(0xA5, 0x38, false), Some(Slot::Mod(Mods::ALT_R)));
    assert_eq!(slot(0xA5, 0x38, true), Some(Slot::Mod(Mods::ALT_R)));
}

/// Punctuation follows the physical position: UK's @ key (VK_OEM_3) at the quote position is Quote, and the ¬ key (VK_OEM_8) at the grave position is Grave.
#[test]
fn punctuation_is_positional() {
    assert_eq!(slot(0xC0, 0x28, false), Some(Slot::Key(Key::Quote)));
    assert_eq!(slot(0xDF, 0x29, false), Some(Slot::Key(Key::Grave)));
    assert_eq!(slot(0xBF, 0x35, false), Some(Slot::Key(Key::Slash)));
}

/// Letters, F-keys and keypad Enter use the virtual key and the extended flag.
#[test]
fn other_keys_use_the_virtual_key() {
    assert_eq!(slot(0x47, 0x22, false), Some(Slot::Key(Key::G)));
    assert_eq!(slot(0x77, 0x42, false), Some(Slot::Key(Key::F8)));
    assert_eq!(slot(0x0D, 0x1C, false), Some(Slot::Key(Key::Return)));
    assert_eq!(slot(0x0D, 0x1C, true), Some(Slot::Key(Key::KeypadEnter)));
    assert_eq!(slot(0x03, 0x46, false), Some(Slot::Key(Key::Pause)));
    assert_eq!(slot(0xFF, 0, false), None);
}

/// Only the Left Ctrl with the AltGr scan code is a phantom.
#[test]
fn the_altgr_phantom_is_recognised() {
    assert!(is_phantom_ctrl(0xA2, 0x21D));
    assert!(!is_phantom_ctrl(0xA2, 0x1D));
    assert!(!is_phantom_ctrl(0xA3, 0x21D));
}
