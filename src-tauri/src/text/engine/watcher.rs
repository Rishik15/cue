//! File: text/engine/watcher.rs
//! Purpose: The idle watcher of the engine: one thread per running server generation that waits on a deadline, stops the server when it has been idle long enough, and retires without touching anything when its generation is gone.
//! Contents: ensure_watcher — start the watcher when the setting wants one and none serves this generation; watch — the thread body; idle_step — what the watcher does now (pure).
//! shortcut: when the watcher thread cannot start, the server simply stays loaded until the next lease or release tries again (`ensure_watcher` runs on both); a dying process is the only cause, so no further fallback.

use std::sync::Arc;
use std::time::Duration;

use super::seams::{Clock, Host};
use super::{Engine, Phase, State};

/// What the watcher of a generation does next.
enum Idle {
    Retire,
    Expired,
    Wait(Duration),
}

impl<H: Host, C: Clock> Engine<H, C> {
    /// Starts the one idle watcher for the running server when the setting wants one and none serves this generation. Input: the locked state. Returns nothing.
    pub(super) fn ensure_watcher(self: &Arc<Self>, st: &mut State<H::Child>) {
        if self.host.idle_secs() <= 0 || st.watcher == Some(st.gen) || !matches!(st.phase, Phase::Running(_)) {
            return;
        }
        let (engine, gen) = (Arc::clone(self), st.gen);
        if self.host.spawn(Box::new(move || engine.watch(gen))) {
            st.watcher = Some(gen);
        }
    }

    /// The watcher thread for generation `gen`: waits on a deadline, stops the server when it has been idle long enough, and retires without touching anything when the generation is no longer the running one.
    fn watch(self: &Arc<Self>, gen: u64) {
        let mut st = self.lock();
        loop {
            match self.idle_step(&st, gen) {
                Idle::Wait(d) => st = self.clock.wait(&self.changed, st, d),
                Idle::Expired => {
                    drop(self.retire(st));
                    return;
                }
                Idle::Retire => {
                    if st.watcher == Some(gen) {
                        st.watcher = None;
                    }
                    return;
                }
            }
        }
    }

    /// What the watcher of `gen` does now: retire (not the running generation, or the setting no longer wants a timer), stop (idle long enough and free) or wait. Inputs: the locked state and the generation. Returns the step.
    fn idle_step(&self, st: &State<H::Child>, gen: u64) -> Idle {
        let Phase::Running(live) = &st.phase else { return Idle::Retire };
        let secs = self.host.idle_secs();
        if st.gen != gen || secs <= 0 {
            return Idle::Retire;
        }
        let idle = Duration::from_secs(secs.unsigned_abs());
        if live.leases > 0 {
            return Idle::Wait(idle);
        }
        let since = self.clock.now().saturating_sub(live.last_used);
        if since >= idle {
            Idle::Expired
        } else {
            Idle::Wait(idle - since)
        }
    }
}
