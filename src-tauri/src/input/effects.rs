//! File: input/effects.rs
//! Purpose: What the input thread asks others to do, and the rule that decides whether a queued action still runs. The hook thread only builds these; the effects thread (or a test) carries them out.
//! Contents: Effect — an action edge, an observer signal, or a view to publish; Sink — where effects go (false when the slot could not be reserved); Gate — drops a start whose generation went stale and always lets a started action be released, even one whose release could not be queued (`owe`, `owed_releases`).

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use super::view::{CatalogView, Hub, RecorderView};

/// One thing for the effects side to do.
pub enum Effect {
    Action { idx: u8, down: bool, gen: u64 },
    Escape,
    Typing,
    Check,
    Report(String),
    MenuMask,
    Recorder(Arc<Hub<RecorderView>>, RecorderView),
    Catalog(Arc<Hub<CatalogView>>, CatalogView),
}

/// Takes effects from the input thread. `send` must never block; false means the effect was not taken. `wake` asks the other side to look at owed releases even when it has nothing queued.
pub trait Sink: Send {
    fn send(&mut self, effect: Effect) -> bool;

    /// Wakes the effects side without queuing work; the default does nothing.
    fn wake(&mut self) {}
}

impl Sink for Box<dyn Sink> {
    /// Forwards to the boxed sink.
    fn send(&mut self, effect: Effect) -> bool {
        self.as_mut().send(effect)
    }

    /// Forwards to the boxed sink.
    fn wake(&mut self) {
        self.as_mut().wake();
    }
}

/// The current generation of every shortcut, bumped on disable, rebind, recording start and recovery.
pub struct Gens {
    gens: Vec<AtomicU64>,
    owed: Vec<AtomicBool>,
}

impl Gens {
    /// Generations for `n` shortcuts, all zero.
    pub fn new(n: usize) -> Arc<Gens> {
        Arc::new(Gens { gens: (0..n).map(|_| AtomicU64::new(0)).collect(), owed: (0..n).map(|_| AtomicBool::new(false)).collect() })
    }

    /// The generation of shortcut `i`.
    pub fn get(&self, i: usize) -> u64 {
        self.gens.get(i).map_or(0, |g| g.load(Ordering::Acquire))
    }

    /// Moves shortcut `i` to a new generation and returns it.
    pub fn bump(&self, i: usize) -> u64 {
        self.gens.get(i).map_or(0, |g| g.fetch_add(1, Ordering::AcqRel) + 1)
    }

    /// Records that the release of shortcut `i` could not be queued, so the effects side owes it.
    pub fn owe(&self, i: usize) {
        if let Some(flag) = self.owed.get(i) {
            flag.store(true, Ordering::Release);
        }
    }

    /// Whether shortcut `i` owed a release, clearing the debt.
    fn take_owed(&self, i: usize) -> bool {
        self.owed.get(i).is_some_and(|flag| flag.swap(false, Ordering::AcqRel))
    }
}

/// Decides, per shortcut, whether an action edge runs. A start from an old generation is dropped; a release always runs once if the start ran, so a feature is never left started.
pub struct Gate {
    gens: Arc<Gens>,
    started: Vec<bool>,
}

impl Gate {
    /// A gate over `gens` with nothing started.
    pub fn new(gens: Arc<Gens>, n: usize) -> Gate {
        Gate { gens, started: vec![false; n] }
    }

    /// Whether the edge should run now.
    pub fn admit(&mut self, idx: usize, down: bool, gen: u64) -> bool {
        let Some(started) = self.started.get_mut(idx) else { return false };
        if down {
            let fresh = gen >= self.gens.get(idx);
            *started = *started || fresh;
            return fresh;
        }
        std::mem::take(started)
    }

    /// The shortcuts whose release could not be queued and whose start did run: each is released here, once.
    pub fn owed_releases(&mut self) -> Vec<usize> {
        let mut due = Vec::new();
        for idx in 0..self.started.len() {
            if self.gens.take_owed(idx) && std::mem::take(&mut self.started[idx]) {
                due.push(idx);
            }
        }
        due
    }
}
