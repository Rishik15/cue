//! File: snippets/expand/flow.rs
//! Purpose: What one typed key does to the expander, as a pure value: held for a running expansion, an undo, a new expansion, or nothing. It also ends an expansion and replays the keys typed meanwhile in the same call, so the caller's one lock covers the move to Idle and the whole replay and no newer key can slip between older ones.
//! Contents: Flow — tracker, stage (`Phase`) and the undo that may be taken back; Next — what the caller must start; `key` and `end`.
//! No OS call and no lock live here. The caller holds one mutex across each call and starts the work `Next` names after releasing it.

use super::matcher::{Hit, Matcher, Tracker};
use super::phase::{Late, Phase};
use crate::platform::Typed;

/// What the caller must start after a key or the end of an expansion.
#[derive(Debug, PartialEq, Eq)]
pub enum Next<U> {
    /// The key belongs to the running expansion and was kept.
    Held,
    /// Nothing to start.
    Nothing,
    /// Take the last expansion back; the stage is already busy.
    Undo(U),
    /// Expand this trigger; the stage is already busy.
    Expand(Hit),
}

/// The tracker of typed characters, the stage of the running expansion and the expansion that Backspace may still take back (`U`).
pub struct Flow<U> {
    pub tracker: Tracker,
    pub phase: Phase,
    pub undo: Option<U>,
}

impl<U> Default for Flow<U> {
    /// A flow with nothing typed, nothing running and nothing to undo.
    fn default() -> Self {
        Flow { tracker: Tracker::default(), phase: Phase::default(), undo: None }
    }
}

impl<U> Flow<U> {
    /// One typed key. Input: the key and the triggers. Returns what to start: a key during an expansion is only kept; otherwise Backspace takes back the armed undo, and any other key forgets it and may complete a trigger.
    pub fn key(&mut self, key: Typed, matcher: &Matcher) -> Next<U> {
        if self.phase.push(key) {
            return Next::Held;
        }
        if let (Some(undo), Typed::Backspace) = (self.undo.take(), key) {
            self.phase.begin();
            return Next::Undo(undo);
        }
        match self.tracker.feed(key, matcher) {
            Some(hit) => {
                self.phase.begin();
                Next::Expand(hit)
            }
            None => Next::Nothing,
        }
    }

    /// Ends the running expansion, forgets what was typed before it and feeds the keys typed meanwhile through `key`, all in this call. Input: whether the target's content is known, and the triggers. Returns what to start (a replayed key may complete a trigger; the keys after it are then held for it) and how many keys were withheld because delivery failed.
    pub fn end(&mut self, delivered: bool, matcher: &Matcher) -> (Next<U>, usize) {
        self.tracker.clear();
        match self.phase.finish(delivered) {
            Late::Withheld(n) => (Next::Nothing, n),
            Late::Replay(keys) => {
                let next = keys.into_iter().fold(Next::Nothing, |next, key| match self.key(key, matcher) {
                    Next::Held | Next::Nothing => next,
                    started => started,
                });
                (next, 0)
            }
        }
    }
}
