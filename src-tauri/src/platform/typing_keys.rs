//! File: platform/typing_keys.rs
//! Purpose: Decides what Backspace, Enter, Tab and Space mean to the typing watcher given the modifiers held, as pure logic testable without Windows. A chord (Ctrl, Alt without Ctrl, Win) or Shift+Tab is never a typed delimiter or Backspace: it breaks the word.
//! Contents: Held — the modifiers down; is_chord — whether they make a shortcut; Special — the four keys; classify — key and modifiers to a `Typed`.

use super::Typed;

/// Which modifier keys are down.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Held {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub win: bool,
}

/// Whether the modifiers make a shortcut rather than text: Win, or Ctrl xor Alt (both together are AltGr, which types). Input: the held modifiers. Returns true for a shortcut.
pub(crate) fn is_chord(h: Held) -> bool {
    h.win || h.ctrl != h.alt
}

/// The keys whose meaning does not come from the keyboard layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Special {
    Back,
    Return,
    Tab,
    Space,
}

/// What the key means with the modifiers held: Reset for a chord or Shift+Tab (focus moves, nothing is typed), Backspace for Back, else the delimiter character. Inputs: the key and the held modifiers.
pub(crate) fn classify(key: Special, h: Held) -> Typed {
    if is_chord(h) || (key == Special::Tab && h.shift) {
        return Typed::Reset;
    }
    match key {
        Special::Back => Typed::Backspace,
        Special::Return => Typed::Char('\n'),
        Special::Tab => Typed::Char('\t'),
        Special::Space => Typed::Char(' '),
    }
}
