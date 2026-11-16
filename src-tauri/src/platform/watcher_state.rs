//! File: platform/watcher_state.rs
//! Purpose: The life of a watcher thread as a pure state machine, so start, timeout, failure and stop can be replayed in a test without Windows. Every start gets a generation; an event for an older generation can never revive, stop or clear a newer watcher, and a start that timed out keeps its ownership (Stopping) until its thread has cleaned up, so a replacement never shares the thread's process-wide resources.
//! Contents: State — Stopped, Starting, Running, Stopping; Event — what the adapter observed; Effect — what the adapter must do; Machine — `apply` one event.
//! No thread, window, channel or clock lives here: `win/watcher.rs` holds the machine under a short mutex (never across an OS call), turns what Windows reports into events and carries out the effects.

/// Where one watcher stands. The number is the generation of the start that made it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Stopped,
    Starting(u64),
    Running(u64),
    Stopping(u64),
}

/// What the adapter observed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    Start,
    /// Generation `n`'s thread, with its thread id, has made its window.
    Ready(u64, u32),
    StartFailed(u64),
    StartTimedOut(u64),
    Stop,
    QuitFailed(u64),
    Ended(u64),
}

/// What the adapter must do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    /// Start generation `n`'s thread and wait for its answer.
    Spawn(u64),
    /// Generation `n` is accepted: its thread runs its message loop, and a start waiting for it succeeds.
    Proceed(u64),
    /// Generation `n` is not wanted (timed out, stopped, or stale): its thread cleans up and returns without a message loop.
    Abandon(u64),
    /// Ask generation `n`'s message loop, on this thread id, to end.
    Quit(u64, u32),
    /// A start cannot be served now: the old watcher is still closing.
    Refuse,
}

/// The state and the last generation handed out.
#[derive(Debug)]
pub struct Machine {
    state: State,
    issued: u64,
    /// The accepted thread's id; 0 when no watcher is accepted.
    thread: u32,
}

impl Machine {
    /// A machine with no watcher.
    pub const fn new() -> Machine {
        Machine { state: State::Stopped, issued: 0, thread: 0 }
    }

    /// The current state.
    pub fn state(&self) -> State {
        self.state
    }

    /// Takes one event, moves to the next state and returns what the adapter must do. An event for a generation that is not the current one changes nothing, except that a thread's late `Ready` is told to abandon. The thread id is kept from `Ready` until the machine is Stopped; a timeout that finds the watcher running changes nothing.
    pub fn apply(&mut self, event: Event) -> Vec<Effect> {
        use Effect::{Abandon, Proceed, Quit, Refuse, Spawn};
        use State::{Running, Starting, Stopped, Stopping};
        let (next, effects) = match (self.state, event) {
            (Stopped, Event::Start) => {
                self.issued += 1;
                (Starting(self.issued), vec![Spawn(self.issued)])
            }
            (Stopping(_), Event::Start) => (self.state, vec![Refuse]),
            (Starting(g), Event::Ready(h, thread)) if g == h => {
                self.thread = thread;
                (Running(g), vec![Proceed(g)])
            }
            (_, Event::Ready(h, _)) => (self.state, vec![Abandon(h)]),
            (Starting(g) | Stopping(g), Event::StartFailed(h)) if g == h => (Stopped, vec![]),
            (Starting(g), Event::StartTimedOut(h)) if g == h => (Stopping(g), vec![]),
            (Running(g), Event::StartTimedOut(h)) if g == h => (self.state, vec![Proceed(g)]),
            (Running(g), Event::Stop) => (Stopping(g), vec![Quit(g, self.thread)]),
            (Starting(g), Event::Stop) => (Stopping(g), vec![]),
            (Stopping(g), Event::QuitFailed(h)) if g == h => (Running(g), vec![]),
            (Starting(g) | Running(g) | Stopping(g), Event::Ended(h)) if g == h => (Stopped, vec![]),
            _ => (self.state, vec![]),
        };
        self.state = next;
        if next == Stopped {
            self.thread = 0;
        }
        effects
    }
}
