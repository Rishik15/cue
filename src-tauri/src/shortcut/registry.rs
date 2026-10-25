//! Purpose: The one place every Cue shortcut is defined. Everything else (matching, recording, display, persistence) works off a shortcut's name, so a new feature adds one `Spec` here and one row on the Shortcuts page, nothing more.
//! Contents: Act — what a shortcut does (Press on key down, or Hold for push-to-talk that acts on both edges); Spec — a shortcut's name, label, default combination and action; SHORTCUTS — the table; N — how many; spec / default_hotkey / label — look one up.

use handy_keys::{Hotkey, Key, Modifiers};
use tauri::AppHandle;

use super::{DEFINE, DICTATION, GRAB, NOTES, SELECTION};

/// How a shortcut's feature is driven. `Hold` runs on press and release (dictation's push-to-talk); `Press` runs once on key down.
pub(super) enum Act {
    Press(fn(&AppHandle)),
    Hold(fn(&AppHandle, bool)),
}

pub(super) struct Spec {
    pub name: &'static str,
    /// What the Shortcuts page calls it, used in a clash message ("Already used by Grab Text").
    pub label: &'static str,
    pub default: (Modifiers, Key),
    pub act: Act,
}

const CA: Modifiers = Modifiers::CTRL_LEFT.union(Modifiers::OPT_LEFT);

/// Every shortcut Cue has. The order is the order the Shortcuts page shows them.
pub(super) const SHORTCUTS: [Spec; 5] = [
    Spec { name: DICTATION, label: "Dictation", default: (Modifiers::CTRL_LEFT, Key::Space), act: Act::Hold(crate::dictation::on_hotkey) },
    Spec { name: SELECTION, label: "Transform Text", default: (CA, Key::Space), act: Act::Press(crate::selection::on_press) },
    Spec { name: GRAB, label: "Grab Text", default: (CA, Key::G), act: Act::Press(crate::grab::on_press) },
    Spec { name: DEFINE, label: "Dictionary", default: (CA, Key::D), act: Act::Press(crate::dictionary::on_press) },
    Spec { name: NOTES, label: "Notes", default: (CA, Key::N), act: Act::Press(crate::notes::on_press) },
];

pub(super) const N: usize = SHORTCUTS.len();

/// The shortcut named by the UI, or None when the name is unknown.
pub(super) fn spec(name: &str) -> Option<&'static Spec> {
    SHORTCUTS.iter().find(|s| s.name == name)
}

/// The combination a shortcut comes with. The name is one of `SHORTCUTS`, so the default is always valid.
pub fn default_hotkey(name: &str) -> Hotkey {
    let (modifiers, key) = spec(name).map(|s| s.default).unwrap_or((Modifiers::CTRL_LEFT, Key::Space));
    Hotkey::new(modifiers, key).expect("valid default")
}

/// What the Shortcuts page calls a shortcut, for a clash message.
pub(super) fn label(name: &str) -> &'static str {
    spec(name).map(|s| s.label).unwrap_or("Dictation")
}

#[cfg(test)]
mod tests {
    use super::super::keys::usable;
    use super::*;

    #[test]
    fn every_shortcut_has_a_unique_usable_default() {
        for s in &SHORTCUTS {
            assert!(usable(&default_hotkey(s.name)), "{} default is not usable", s.name);
            assert_eq!(SHORTCUTS.iter().filter(|o| o.name == s.name).count(), 1, "{} appears twice", s.name);
            assert_eq!(SHORTCUTS.iter().filter(|o| o.default == s.default).count(), 1, "{} default clashes", s.name);
        }
    }
}
