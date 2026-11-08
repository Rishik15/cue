//! File: input/chord.rs
//! Purpose: A key combination as Cue stores, matches, compares and shows it. The stored shape is the one settings.json already holds (`{"modifiers": bits, "key": name|null}`), a pair of side bits for one family means "either side", and `both` (memory only) means both sides at once.
//! Contents: Chord — the combination with `matches`, `still_held`, `intersects`, `usable`, `reserved` and `parts`; Part — one shown key.

use serde::{Deserialize, Serialize};

use super::key::{Group, Key, Mods, GROUPS};

/// One displayed key: `code` picks the icon, `side` is 'L'/'R' for a side-specific modifier.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Part {
    pub code: String,
    pub label: String,
    pub side: Option<char>,
}

/// A combination: modifiers and an optional main key (a modifier-only combination has none).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Chord {
    pub modifiers: Mods,
    pub key: Option<Key>,
    #[serde(skip)]
    pub both: Mods,
}

/// How a family is held: bit 0 left, bit 1 right.
fn held_state(g: &Group, held: Mods) -> u8 {
    u8::from(held.any(g.left)) | u8::from(held.any(g.right)) << 1
}

impl Chord {
    /// A combination, or None when it has neither a modifier nor a key.
    pub fn new(modifiers: Mods, key: Option<Key>) -> Option<Chord> {
        (!modifiers.is_empty() || key.is_some()).then_some(Chord { modifiers, key, both: Mods::NONE })
    }

    /// Whether a family needs both of its sides.
    fn needs_both(&self, g: &Group) -> bool {
        self.both.has(g.left) && self.both.has(g.right)
    }

    /// Whether the family `g` held as `state` satisfies this combination (the library's rule: a required side must be down, a family the combination does not use must be up).
    fn accepts(&self, g: &Group, state: u8) -> bool {
        let (l, r) = (self.modifiers.has(g.left), self.modifiers.has(g.right));
        match (l, r) {
            _ if self.needs_both(g) => state == 3,
            (true, true) => state != 0,
            (true, false) => state & 1 != 0,
            (false, true) => state & 2 != 0,
            (false, false) => state == 0,
        }
    }

    /// Whether the modifiers held now start this combination: every required side down, no other family down, Fn exact.
    pub fn matches(&self, held: Mods) -> bool {
        GROUPS.iter().all(|g| self.accepts(g, held_state(g, held))) && self.modifiers.has(Mods::FN) == held.has(Mods::FN)
    }

    /// Whether an already-started combination is still held: other families touched meanwhile do not end it.
    pub fn still_held(&self, held: Mods) -> bool {
        let wanted = GROUPS.iter().filter(|g| self.modifiers.any(g.both())).fold(Mods::FN, |all, g| all | g.both());
        self.matches(held.without(Mods(Mods::KNOWN).without(wanted)))
    }

    /// Whether some held state satisfies both combinations with the same main key (the real conflict test, not structural equality).
    pub fn intersects(&self, other: &Chord) -> bool {
        self.key == other.key
            && self.modifiers.has(Mods::FN) == other.modifiers.has(Mods::FN)
            && GROUPS.iter().all(|g| (0..4).any(|s| self.accepts(g, s) && other.accepts(g, s)))
    }

    /// Whether one is modifier-only and its families are all among the other's: pressing the longer one passes through the shorter, which would fire too.
    pub fn nests(&self, other: &Chord) -> bool {
        let within = |small: &Chord, big: &Chord| {
            small.key.is_none() && GROUPS.iter().filter(|g| small.modifiers.any(g.both())).all(|g| big.modifiers.any(g.both()))
        };
        within(self, other) || within(other, self)
    }

    /// A bare non-F-key would hijack typing; modifiers alone fire on every Ctrl or Shift tap unless there are two of them or one is Alt or Win.
    pub fn usable(&self) -> bool {
        if let Some(k) = self.key {
            return !self.modifiers.is_empty() || k.is_function();
        }
        let kinds = GROUPS.iter().map(Group::both).chain([Mods::FN]);
        kinds.filter(|k| self.modifiers.any(*k)).count() >= 2 || self.modifiers.any(GROUPS[1].both() | GROUPS[3].both())
    }

    /// Whether the combination is one Cue keeps for itself: a bare Escape is how every surface is left.
    pub fn reserved(&self) -> bool {
        self.key == Some(Key::Escape) && self.modifiers.is_empty()
    }

    /// The keys to draw. Win+Shift+F23 is what the Copilot key sends, shown as one key.
    pub fn parts(&self) -> Vec<Part> {
        let m = self.modifiers;
        if m.any(GROUPS[3].both()) && m.any(GROUPS[2].both()) && self.key == Some(Key::F23) {
            return vec![Part { code: "copilot".into(), label: "Copilot".into(), side: None }];
        }
        let mut parts: Vec<Part> = GROUPS.iter().filter_map(|g| group_part(g, m)).collect();
        if m.has(Mods::FN) {
            parts.push(Part { code: "fn".into(), label: "Fn".into(), side: None });
        }
        parts.extend(self.key.map(|k| Part { code: k.label().to_lowercase(), label: k.label().into(), side: None }));
        parts
    }
}

/// The part for one family, or None when the combination does not use it. A single side is marked L or R.
fn group_part(g: &Group, m: Mods) -> Option<Part> {
    let side = match (m.has(g.left), m.has(g.right)) {
        (true, false) => Some('L'),
        (false, true) => Some('R'),
        (true, true) => None,
        _ => return None,
    };
    Some(Part { code: g.code.into(), label: g.label.into(), side })
}
