//! File: input/ledger.rs
//! Purpose: The per-key ledger: which keys are physically down and who owns each press. It separates a physical edge from a down delivered to the app and a down Cue captured, so a captured key's repeats and release are still swallowed after its owner is gone, and a key held before a surface opened keeps passing. Fixed size, no allocation.
//! Contents: Owner — who a press belongs to; Slot — a key or a sided modifier; Ledger — the table with `press`, `release`, `held_mods`, `uncapture_all` and `forget_all`.

use super::key::{Key, Mods};

/// Who a physical press belongs to; its repeats and release go the same way.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Owner {
    #[default]
    Pass,
    Drain,
    Popup(u64),
    Recorder(u64),
    Action(u8),
}

impl Owner {
    /// Whether the press was swallowed.
    pub fn captured(self) -> bool {
        !matches!(self, Owner::Pass)
    }
}

/// One key or one sided modifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Key(Key),
    Mod(Mods),
}

#[derive(Clone, Copy, Default)]
struct Entry {
    down: bool,
    owner: Owner,
}

const MODS: usize = 9;

/// Physical state of every key.
pub struct Ledger {
    keys: [Entry; Key::COUNT],
    mods: [Entry; MODS],
}

impl Default for Ledger {
    /// An empty ledger.
    fn default() -> Ledger {
        Ledger::new()
    }
}

impl Ledger {
    /// An empty ledger: nothing is down.
    pub fn new() -> Ledger {
        Ledger { keys: [Entry::default(); Key::COUNT], mods: [Entry::default(); MODS] }
    }

    /// The entry for a slot. A modifier slot holds exactly one bit.
    fn entry(&mut self, slot: Slot) -> &mut Entry {
        match slot {
            Slot::Key(k) => &mut self.keys[k as usize],
            Slot::Mod(m) => &mut self.mods[(m.0.trailing_zeros() as usize).min(MODS - 1)],
        }
    }

    /// Whether the slot is physically down.
    pub fn is_down(&mut self, slot: Slot) -> bool {
        self.entry(slot).down
    }

    /// The owner of a slot's current press.
    pub fn owner(&mut self, slot: Slot) -> Owner {
        self.entry(slot).owner
    }

    /// Marks the slot down with `owner`.
    pub fn press(&mut self, slot: Slot, owner: Owner) {
        *self.entry(slot) = Entry { down: true, owner };
    }

    /// Marks the slot up and returns who owned the press.
    pub fn release(&mut self, slot: Slot) -> Owner {
        std::mem::take(self.entry(slot)).owner
    }

    /// Hands a still-held press to another owner (a lease that ended turns its keys into drains).
    pub fn reassign(&mut self, from: Owner, to: Owner) {
        for e in self.keys.iter_mut().chain(self.mods.iter_mut()) {
            if e.down && e.owner == from {
                e.owner = to;
            }
        }
    }

    /// The modifiers physically down now.
    pub fn held_mods(&self) -> Mods {
        self.mods.iter().enumerate().filter(|(_, e)| e.down).fold(Mods::NONE, |all, (bit, _)| all | Mods(1 << bit))
    }

    /// The modifiers physically down whose press passed to the app (not captured).
    pub fn passed_mods(&self) -> Mods {
        self.mods
            .iter()
            .enumerate()
            .filter(|(_, e)| e.down && e.owner == Owner::Pass)
            .fold(Mods::NONE, |all, (bit, _)| all | Mods(1 << bit))
    }

    /// Whether any key or modifier is down.
    pub fn any_down(&self) -> bool {
        self.keys.iter().chain(self.mods.iter()).any(|e| e.down)
    }

    /// Stops swallowing: every captured press now passes its repeats and release. Used on error and expiry, where a stuck capture is worse than a stray key.
    pub fn uncapture_all(&mut self) {
        for e in self.keys.iter_mut().chain(self.mods.iter_mut()) {
            e.owner = Owner::Pass;
        }
    }

    /// Forgets everything (the session changed, so releases may have been missed).
    pub fn forget_all(&mut self) {
        *self = Ledger::new();
    }
}
