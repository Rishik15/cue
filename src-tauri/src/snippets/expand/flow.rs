//! File: snippets/expand/flow.rs
//! Purpose: What one typed key does to the expander, as a pure value: held for a running expansion, an undo, a new expansion, or nothing. It also ends an expansion and replays the keys typed meanwhile in the same call, so the caller's one lock covers the move to Idle and the whole replay and no newer key can slip between older ones.
//! Contents: Flow — the one owner of the tracker, the stage (`Phase`) and the undo that may be taken back; Next — what the caller must start; Outcome — how a worker ended; `key`, `extend` (after an injection delay), `seal`, `end`, `replace_index` and `stop`.
//! No OS call and no lock live here. The caller holds one mutex across each call and starts the work `Next` names after releasing it. Pending work is tied to a generation: swapping the triggers or stopping the expander makes the running worker stale, so its `seal` or `current` refuses (nothing is typed or undone) and its `end` never arms an undo (and, after a stop, never replays keys or starts anything).

use super::matcher::{Hit, Matcher, Tracker};
use super::phase::{Late, Phase};
use super::settle::settle;
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

/// How a worker ended: the text went in (and the expansion Backspace may take back, when it qualifies) or typing failed part way, leaving the target's content unknown.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome<U> {
    Delivered(Option<U>),
    Failed,
}

/// The tracker of typed characters, the stage of the running expansion and the expansion that Backspace may still take back (`U`), all private: callers move them only through the methods.
pub struct Flow<U> {
    tracker: Tracker,
    phase: Phase,
    undo: Option<U>,
    /// Changes whenever the triggers are swapped or the expander stops; work begun under an older one is stale.
    generation: u64,
    /// The generation the running expansion began under.
    began: u64,
    stopped: bool,
}

impl<U> Default for Flow<U> {
    /// A flow with nothing typed, nothing running and nothing to undo.
    fn default() -> Self {
        Flow { tracker: Tracker::default(), phase: Phase::default(), undo: None, generation: 0, began: 0, stopped: false }
    }
}

impl<U> Flow<U> {
    /// Starts an expansion under the current generation.
    fn begin(&mut self) {
        self.phase.begin();
        self.began = self.generation;
    }

    /// Whether the running expansion or undo still belongs to the triggers in force: false once the expander stopped or the triggers were swapped since it began. Workers ask this right before they change the target.
    pub fn current(&self) -> bool {
        !self.stopped && self.began == self.generation
    }

    /// The triggers were replaced: forgets what was typed and the armed undo, and makes the running expansion stale (its keys stay held and are replayed by `end` through the new triggers).
    pub fn replace_index(&mut self) {
        self.tracker.clear();
        self.undo = None;
        self.generation += 1;
    }

    /// The expander was stopped: nothing is armed, the running expansion is stale, and no key starts anything any more.
    pub fn stop(&mut self) {
        self.replace_index();
        self.stopped = true;
    }

    /// Takes the keys typed since the trigger and settles them, for the running expansion (see `Phase::seal`). Returns the text typed meanwhile, or None when the expansion must not go ahead: it is stale, or a key during it forbids it.
    pub fn seal(&mut self) -> Option<String> {
        if !self.current() {
            return None;
        }
        self.phase.seal()
    }

    /// One typed key. Input: the key and the triggers. Returns what to start: a key during an expansion is only kept; otherwise Backspace takes back the armed undo, and any other key forgets it and may complete a trigger.
    pub fn key(&mut self, key: Typed, matcher: &Matcher) -> Next<U> {
        if self.stopped {
            return Next::Nothing;
        }
        if self.phase.push(key) {
            return Next::Held;
        }
        if let (Some(undo), Typed::Backspace) = (self.undo.take(), key) {
            self.begin();
            return Next::Undo(undo);
        }
        match self.tracker.feed(key, matcher) {
            Some(hit) => {
                self.begin();
                Next::Expand(hit)
            }
            None => Next::Nothing,
        }
    }

    /// After an injection delay: a key typed during the wait that completes a longer trigger spanning the waiting one (`@@p` after `@@`, not `@@` typed again after other keys) replaces the waiting expansion. Input: the triggers and the waiting trigger. Returns that expansion, which keeps the keys after it held, or None when the waiting one goes ahead: the typed keys complete nothing, complete an unrelated trigger (they stay its `meanwhile` text), or include a caret move or Enter (the delivery is cancelled at the seal).
    pub fn extend(&mut self, matcher: &Matcher, waiting: &Hit) -> Option<Next<U>> {
        if !self.current() {
            return None;
        }
        let Phase::Preparing(keys) = &self.phase else { return None };
        let keys = keys.clone();
        settle(&keys)?;
        self.tracker.restore(matcher, waiting);
        let at_hit = keys.iter().enumerate().find_map(|(at, key)| self.tracker.feed(*key, matcher).map(|hit| (at, hit)));
        let (at, hit) = at_hit
            .filter(|(at, hit)| hit.erase > waiting.erase + at && hit.typed.to_lowercase().starts_with(&waiting.typed.to_lowercase()))?;
        self.phase = Phase::Preparing(keys[at + 1..].to_vec());
        Some(Next::Expand(hit))
    }

    /// Ends the running expansion, forgets what was typed before it, arms the undo a delivery brought and feeds the keys typed meanwhile through `key`, all in this call. Input: how the worker ended and the triggers. Returns what to start (a replayed key may complete a trigger; the keys after it are then held for it) and how many keys were withheld because delivery failed. A stale expansion arms no undo, and after a stop nothing is replayed.
    pub fn end(&mut self, outcome: Outcome<U>, matcher: &Matcher) -> (Next<U>, usize) {
        let was_current = self.current();
        self.tracker.clear();
        let delivered = matches!(outcome, Outcome::Delivered(_));
        let late = self.phase.finish(delivered);
        if self.stopped {
            return (Next::Nothing, 0);
        }
        if let (Outcome::Delivered(undo), true) = (outcome, was_current) {
            self.undo = undo;
        }
        match late {
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
