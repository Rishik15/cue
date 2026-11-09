//! File: input/catalog.rs
//! Purpose: The one place every Cue shortcut is defined. A new shortcut is one `Spec` row here and one row on the Shortcuts page, which draws from the catalog snapshot.
//! Contents: Run — what a shortcut does (Press on key down, or Hold on both edges); Spec — id, label, default and action; Catalog — the specs plus the observers of global Escape and typing; SPECS / CATALOG — Cue's five shortcuts.

use tauri::AppHandle;

use super::key::{Key, Mods};

/// How a shortcut's feature is driven.
#[derive(Clone, Copy)]
pub enum Run {
    Press(fn(&AppHandle)),
    Hold(fn(&AppHandle, bool)),
}

/// One shortcut: its stored name, what the page calls it, its default combination and its action.
pub struct Spec {
    pub id: &'static str,
    pub label: &'static str,
    pub default: (Mods, Key),
    pub run: Run,
}

impl Spec {
    /// A shortcut whose feature runs once on key down.
    pub const fn press(id: &'static str, label: &'static str, default: (Mods, Key), run: fn(&AppHandle)) -> Spec {
        Spec { id, label, default, run: Run::Press(run) }
    }

    /// A shortcut whose feature runs on key down and on release (push to talk).
    pub const fn hold(id: &'static str, label: &'static str, default: (Mods, Key), run: fn(&AppHandle, bool)) -> Spec {
        Spec { id, label, default, run: Run::Hold(run) }
    }
}

/// The shortcuts and the signals sent to features that watch the keyboard without owning a shortcut: `on_escape` runs on every Escape press, `on_key_down` on a typed key that is not a shortcut key, but only while `typing_wanted` says some listener is waiting, so an idle Cue does no work for ordinary typing. Each must return at once.
pub struct Catalog {
    pub specs: &'static [Spec],
    pub on_escape: &'static [fn()],
    pub on_key_down: &'static [fn()],
    pub typing_wanted: fn() -> bool,
}

const CA: Mods = Mods(Mods::CTRL_L.0 | Mods::ALT_L.0);

pub const DICTATION: &str = "dictation";
pub const SELECTION: &str = "selection";
pub const GRAB: &str = "grab";
pub const DEFINE: &str = "define";
pub const NOTES: &str = "notes";

pub const SPECS: [Spec; 5] = [
    Spec::hold(DICTATION, "Dictation", (Mods::CTRL_L, Key::Space), crate::dictation::on_hotkey),
    Spec::press(SELECTION, "Transform Text", (CA, Key::Space), crate::selection::on_press),
    Spec::press(GRAB, "Grab Text", (CA, Key::G), crate::grab::on_press),
    Spec::press(DEFINE, "Dictionary", (CA, Key::D), crate::dictionary::on_press),
    Spec::press(NOTES, "Notes", (CA, Key::N), crate::notes::on_press),
];

const _: () = assert!(
    SPECS.len() <= u8::MAX as usize,
    "shortcut and change indexes are u8: more than 255 shortcuts need wider indexes (input/route.rs, ledger.rs)"
);

pub static CATALOG: Catalog = Catalog {
    specs: &SPECS,
    on_escape: &[crate::dictation::cancel, crate::selection::cancel, crate::dictionary::cancel, crate::grab::cancel],
    on_key_down: &[crate::dictionary::dismiss_on_typing],
    typing_wanted: crate::dictionary::wants_typing,
};
