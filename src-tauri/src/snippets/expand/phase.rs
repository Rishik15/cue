//! File: snippets/expand/phase.rs
//! Purpose: The stage of one expansion as a pure value, so the keys typed while it runs are never lost to a race: Idle, Preparing (keys are collected) and Committing (the batch is sealed and being typed; later keys wait). No OS calls and no lock live here; the caller holds one mutex across each call.
//! Contents: Phase — the stage and the keys it holds; `begin`, `push`, `seal` (drain, settle and the stage change in one call), `has_late` and `finish`.
//! Late keys (typed after the seal) are kept and handed back by `finish`, to be fed through the normal typing path once the expansion was delivered. After a failed delivery the target's content is unknown, so they are handed back as a count only and never replayed.

use super::settle::settle;
use crate::platform::Typed;

/// Where one expansion stands, and the keys the user typed meanwhile.
#[derive(Debug, Default, PartialEq, Eq)]
pub enum Phase {
    #[default]
    Idle,
    Preparing(Vec<Typed>),
    Committing(Vec<Typed>),
}

/// What `finish` hands back: keys to feed through the typing path again, or how many were kept out because delivery was uncertain.
#[derive(Debug, PartialEq, Eq)]
pub enum Late {
    Replay(Vec<Typed>),
    Withheld(usize),
}

impl Phase {
    /// Whether an expansion is under way, so a new key belongs to it.
    pub fn is_busy(&self) -> bool {
        *self != Phase::Idle
    }

    /// Starts an expansion: Idle becomes Preparing with no keys. Returns false, changing nothing, when one is already under way.
    pub fn begin(&mut self) -> bool {
        if self.is_busy() {
            return false;
        }
        *self = Phase::Preparing(Vec::new());
        true
    }

    /// Adds one key typed during the expansion, after the keys already held. Returns false, dropping nothing, when idle (the key is not this expansion's).
    pub fn push(&mut self, key: Typed) -> bool {
        match self {
            Phase::Preparing(keys) | Phase::Committing(keys) => keys.push(key),
            Phase::Idle => return false,
        }
        true
    }

    /// Takes every key collected so far, works out the text typed meanwhile and moves to Committing with an empty late list, all in this one call. None when the expansion must not go ahead (see `settle`) or when not Preparing; the phase is Committing either way once it was Preparing, so later keys are kept.
    pub fn seal(&mut self) -> Option<String> {
        let Phase::Preparing(keys) = self else { return None };
        let settled = settle(keys);
        *self = Phase::Committing(Vec::new());
        settled
    }

    /// Whether a key arrived after the seal: Committing with at least one key held. Such a key already sits in the app, so a batch whose erase count was fixed at the seal must not go out.
    pub fn has_late(&self) -> bool {
        matches!(self, Phase::Committing(keys) if !keys.is_empty())
    }

    /// Ends the expansion and returns to Idle. Keys held are given back: all to replay when `delivered` (nothing was typed, or all of it was), only their count when not, because a partly delivered text leaves the target unknown and replaying could duplicate input.
    pub fn finish(&mut self, delivered: bool) -> Late {
        let keys = match std::mem::take(self) {
            Phase::Preparing(keys) | Phase::Committing(keys) => keys,
            Phase::Idle => Vec::new(),
        };
        if delivered {
            Late::Replay(keys)
        } else {
            Late::Withheld(keys.len())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Phase;
    use crate::platform::Typed::Char;

    /// Only Committing with a key held has a late key; keys held before the seal are not late.
    #[test]
    fn only_a_key_after_the_seal_is_late() {
        assert!(!Phase::Idle.has_late());
        assert!(!Phase::Preparing(vec![Char('x')]).has_late());
        assert!(!Phase::Committing(Vec::new()).has_late());
        assert!(Phase::Committing(vec![Char('x')]).has_late());
    }
}
