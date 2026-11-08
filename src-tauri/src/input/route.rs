//! File: input/route.rs
//! Purpose: Which global shortcuts an edge starts or ends. Pure and per edge: the held modifiers come from the physical ledger, never from the OS key state. Matching follows the library's rules Cue has always used (required sided modifiers down, no other family down, an started shortcut survives other modifiers being touched).
//! Contents: Binding — one shortcut's combination and whether it is live; Matcher — the pressed flag per shortcut with `step`, `release` and `release_all`.

use super::chord::Chord;
use super::key::{Key, Mods};

/// One shortcut's combination, whether its stored row passed validation, and whether the feature and the shortcut are both on.
#[derive(Clone, Copy, Debug)]
pub struct Binding {
    pub chord: Chord,
    pub valid: bool,
    pub enabled: bool,
}

impl Binding {
    /// The combination that fires, unless the row is invalid or the shortcut or its feature is off.
    pub fn active(&self) -> Option<Chord> {
        (self.valid && self.enabled).then_some(self.chord)
    }
}

/// A change of one shortcut: its index in the catalog and whether it is down now.
pub type Change = (u8, bool);

/// Whether each shortcut counts as pressed.
pub struct Matcher {
    down: Vec<bool>,
}

impl Matcher {
    /// A matcher with nothing pressed for `n` shortcuts.
    pub fn new(n: usize) -> Matcher {
        Matcher { down: vec![false; n] }
    }

    /// Whether shortcut `i` counts as pressed.
    pub fn is_down(&self, i: usize) -> bool {
        self.down.get(i).copied().unwrap_or(false)
    }

    /// Takes one edge (`key` None for a modifier) with the modifiers held after it, pushes each shortcut that changed to `out`.
    pub fn step(&mut self, bindings: &[Binding], held: Mods, key: Option<Key>, down: bool, out: &mut Vec<Change>) {
        for (i, (b, was)) in bindings.iter().zip(self.down.iter_mut()).enumerate() {
            let now = match b.active() {
                None => false,
                Some(h) => match (h.key, key) {
                    (Some(k), Some(ek)) if k == ek => down && (h.matches(held) || *was && h.still_held(held)),
                    (Some(_), _) => *was && h.still_held(held),
                    (None, None) => h.matches(held) || *was && h.still_held(held),
                    (None, Some(_)) => *was,
                },
            };
            if now != *was {
                *was = now;
                out.push((i as u8, now));
            }
        }
    }

    /// Lets go of shortcut `i`, pushing the release when it was down.
    pub fn release(&mut self, i: usize, out: &mut Vec<Change>) {
        if self.down.get_mut(i).is_some_and(std::mem::take) {
            out.push((i as u8, false));
        }
    }

    /// Lets go of every shortcut that is down.
    pub fn release_all(&mut self, out: &mut Vec<Change>) {
        for i in 0..self.down.len() {
            self.release(i, out);
        }
    }
}
