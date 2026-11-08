//! File: shortcut/tests/rules.rs
//! Purpose: Protect which combinations may become a shortcut and how they are shown: bare keys that would hijack typing are refused, clashes and overlaps are named, defaults are always valid.
//! Contents: tests of `usable`, `view`, `overlaps`, `validate` and the registry defaults.

use handy_keys::{Hotkey, Key, Modifiers};

use crate::shortcut::keys::{usable, view};
use crate::shortcut::matching::{overlaps, validate};
use crate::shortcut::registry::{default_hotkey, label, SHORTCUTS};
use crate::shortcut::{Binding, DEFINE, DICTATION, GRAB};

/// A hotkey from modifiers and key. Inputs: both. Returns it.
fn hotkey(modifiers: Modifiers, key: impl Into<Option<Key>>) -> Hotkey {
    Hotkey::new(modifiers, key).expect("hotkey")
}

/// The default bindings, all on. Returns one binding per shortcut.
fn defaults() -> Vec<Binding> {
    SHORTCUTS.iter().map(|s| Binding { name: s.name, hotkey: default_hotkey(s.name), enabled: true, shortcut_on: true }).collect()
}

/// A bare letter would hijack typing; an F-key alone, a letter with a modifier, two modifiers, or Alt or Win alone are fine; a lone Ctrl, Shift or Fn is not.
#[test]
fn only_combinations_that_do_not_hijack_typing_are_usable() {
    let (ctrl, shift, alt, win) = (Modifiers::CTRL_LEFT, Modifiers::SHIFT_LEFT, Modifiers::OPT_LEFT, Modifiers::CMD_LEFT);
    for (h, ok) in [
        (hotkey(Modifiers::empty(), Key::A), false),
        (hotkey(Modifiers::empty(), Key::F5), true),
        (hotkey(shift, Key::A), true),
        (hotkey(ctrl, None), false),
        (hotkey(shift, None), false),
        (hotkey(Modifiers::FN, None), false),
        (hotkey(ctrl | shift, None), true),
        (hotkey(alt, None), true),
        (hotkey(win, None), true),
    ] {
        assert_eq!(usable(&h), ok, "{h}");
    }
}

/// Keys are shown as parts: side-specific modifiers carry their side, and the Copilot key is one key.
#[test]
fn combinations_are_shown_as_keycaps() {
    let parts = view(&hotkey(Modifiers::CTRL_LEFT | Modifiers::SHIFT_RIGHT, Key::Space));
    assert_eq!(
        parts.iter().map(|p| (p.code.as_str(), p.side)).collect::<Vec<_>>(),
        [("ctrl", Some('L')), ("shift", Some('R')), ("space", None)]
    );
    let copilot = view(&hotkey(Modifiers::CMD_LEFT | Modifiers::SHIFT_LEFT, Key::F23));
    assert_eq!(copilot.len(), 1);
    assert_eq!(copilot[0].label, "Copilot");
}

/// A modifier-only shortcut inside a longer one overlaps it (pressing the long one passes through the short one); unrelated ones do not.
#[test]
fn a_modifier_only_shortcut_inside_a_longer_one_overlaps() {
    let (ctrl, alt) = (Modifiers::CTRL_LEFT, Modifiers::OPT_LEFT);
    assert!(overlaps(&hotkey(ctrl | alt, None), &hotkey(ctrl | alt, Key::G)));
    assert!(overlaps(&hotkey(ctrl | alt, Key::G), &hotkey(ctrl | alt, None)));
    assert!(!overlaps(&hotkey(ctrl, Key::Space), &hotkey(ctrl | alt, Key::Space)), "both have keys");
    assert!(!overlaps(&hotkey(Modifiers::SHIFT_LEFT | alt, None), &hotkey(ctrl, Key::G)));
}

/// Saving checks usable first, then clashes with another shortcut (named by its page label), then overlaps; keeping a shortcut's own combination is fine.
#[test]
fn saving_a_combination_names_the_reason_it_is_refused() {
    let bindings = defaults();
    let refused = validate(&bindings, DICTATION, &hotkey(Modifiers::empty(), Key::A)).expect_err("bare key");
    assert!(refused.starts_with("Add a modifier key"), "{refused}");
    assert_eq!(validate(&bindings, DICTATION, &default_hotkey(GRAB)), Err("Already used by Grab Text.".to_string()));
    let (ctrl, alt) = (Modifiers::CTRL_LEFT, Modifiers::OPT_LEFT);
    assert_eq!(validate(&bindings, DICTATION, &hotkey(ctrl | alt, None)), Err(format!("Overlaps {}.", label(bindings[1].name))));
    assert_eq!(validate(&bindings, DEFINE, &default_hotkey(DEFINE)), Ok(()));
    assert_eq!(validate(&bindings, DEFINE, &hotkey(ctrl | alt, Key::F9)), Ok(()));
}

/// Every shortcut has a valid default that is usable and does not clash or overlap with another.
#[test]
fn the_defaults_are_valid_and_do_not_collide() {
    let bindings = defaults();
    for binding in &bindings {
        assert_eq!(validate(&bindings, binding.name, &binding.hotkey), Ok(()), "{}", binding.name);
    }
}
