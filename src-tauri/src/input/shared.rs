//! File: input/shared.rs
//! Purpose: The few facts other threads may read without asking the input thread: the modifiers physically held, an epoch that changes when the key state was lost, and a count of unmarked physical key edges. All atomics, so reading never blocks the hook.
//! Contents: Shared — the three atomics with their readers and writers; Mark — a point in the activity count; install_global / global / activity_mark — the process-wide copy other modules read.

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;

use super::key::Mods;

/// State published by the input thread for every other thread.
#[derive(Default)]
pub struct Shared {
    held: AtomicU32,
    epoch: AtomicU64,
    activity: AtomicU64,
}

impl Shared {
    /// Publishes the modifiers physically held now.
    pub fn set_held(&self, mods: Mods) {
        self.held.store(mods.0, Ordering::Release);
    }

    /// The modifiers physically held at the last edge.
    pub fn held(&self) -> Mods {
        Mods(self.held.load(Ordering::Acquire))
    }

    /// Starts a new epoch: the key state before it may be wrong (lock, resume, reinstall).
    pub fn new_epoch(&self) {
        self.epoch.fetch_add(1, Ordering::AcqRel);
    }

    /// The current epoch.
    pub fn epoch(&self) -> u64 {
        self.epoch.load(Ordering::Acquire)
    }

    /// Counts one unmarked physical key edge.
    pub fn bump_activity(&self) {
        self.activity.fetch_add(1, Ordering::AcqRel);
    }

    /// A mark at the current activity count.
    pub fn mark(&self) -> Mark {
        Mark(self.activity.load(Ordering::Acquire))
    }

    /// Whether unmarked physical key edges came since `mark`.
    pub fn since(&self, mark: Mark) -> bool {
        self.activity.load(Ordering::Acquire) != mark.0
    }
}

/// A point in the physical key activity, for a caller that wants to know whether the user pressed keys since.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mark(pub u64);

static GLOBAL: std::sync::OnceLock<Arc<Shared>> = std::sync::OnceLock::new();

/// Makes `shared` the one other modules read through `lift` and `activity_mark`. The first call wins.
pub fn install_global(shared: Arc<Shared>) {
    let _ = GLOBAL.set(shared);
}

/// The installed shared state, if input has started.
pub fn global() -> Option<&'static Arc<Shared>> {
    GLOBAL.get()
}

impl Mark {
    /// Whether unmarked physical key edges came since this mark; false when input has not started.
    pub fn physical_keys_since(&self) -> bool {
        global().is_some_and(|s| s.since(*self))
    }
}

/// A mark at the current physical key activity.
pub fn activity_mark() -> Mark {
    global().map_or(Mark(0), |s| s.mark())
}
