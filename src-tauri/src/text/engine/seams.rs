//! File: text/engine/seams.rs
//! Purpose: What the engine needs from the outside, as traits with a real and a scripted side: the server process (`Child`), where processes, settings and threads come from (`Host`), and time (`Clock`); plus the small result types the engine shares.
//! Contents: Conn — how to reach a server; Fail — why a call produced no lease; Probe — a started server's answer to "ready?"; Child; Host; Clock; SystemClock — the real clock; POLL and READY_LIMIT — the engine's two time constants.

use std::sync::{Condvar, MutexGuard};
use std::time::Duration;

/// How often a waiter looks again whether it is still wanted, and how often a cold start asks the server whether it has loaded.
pub const POLL: Duration = Duration::from_millis(100);

/// Longest wait for the model to load. Seconds on an SSD; a hard disk with a 2-thread CPU can need minutes, so the limit is generous.
pub const READY_LIMIT: Duration = Duration::from_secs(240);

/// How to reach the running server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Conn {
    pub port: u16,
    pub key: String,
}

/// Why a call to the server did not produce a lease.
#[derive(Debug, PartialEq, Eq)]
pub enum Fail {
    Cancelled,
    Failed(String),
}

/// What a started server says when asked whether it is ready.
pub enum Probe {
    Loading,
    Ready(Conn),
    Failed(String),
}

/// One server process. `kill` ends it and waits, so its model file is released when it returns.
pub trait Child: Send + 'static {
    /// Whether this is still the live, trusted server (the process runs and owns its port).
    fn alive(&mut self) -> bool;
    /// Asks whether the server has loaded the model yet.
    fn probe(&mut self) -> Probe;
    /// Ends the process and waits for it.
    fn kill(&mut self);
}

/// Where processes, settings and threads come from: the real one starts llama.cpp, a test one is scripted.
pub trait Host: Send + Sync + 'static {
    type Child: Child;
    /// Starts a server for `model` without waiting for it to load; `fallback` asks for the safer way (CPU only). Returns the process or the reason it did not start.
    fn launch(&self, model: &'static str, fallback: bool) -> Result<Self::Child, String>;
    /// Seconds a server stays loaded after its last request: 0 stops it at once, below 0 never.
    fn idle_secs(&self) -> i64;
    /// Whether a failed start is worth one more try with `fallback`.
    fn retry_after_failure(&self) -> bool;
    /// Called when the fallback start worked, so later starts can skip the failing way.
    fn fallback_worked(&self);
    /// Runs `body` on a new thread. Returns false when the thread could not start.
    fn spawn(&self, body: Box<dyn FnOnce() + Send>) -> bool;
    /// Records a line for the support log.
    fn note(&self, line: &str);
}

/// Time as the engine sees it, so tests can move it by hand.
pub trait Clock: Send + Sync + 'static {
    /// Time since an arbitrary start.
    fn now(&self) -> Duration;
    /// Sleeps for `d` (a cold start waits between two questions to the server).
    fn pause(&self, d: Duration);
    /// Waits on `cv` for at most `d`, returning the lock again.
    fn wait<'a, T>(&self, cv: &Condvar, guard: MutexGuard<'a, T>, d: Duration) -> MutexGuard<'a, T>;
}

/// The real clock: `Instant` and the OS sleep.
pub struct SystemClock(std::time::Instant);

impl SystemClock {
    /// A clock whose zero is now.
    pub fn new() -> Self {
        Self(std::time::Instant::now())
    }
}

impl Clock for SystemClock {
    /// Time since the clock was made.
    fn now(&self) -> Duration {
        self.0.elapsed()
    }

    /// Sleeps the thread.
    fn pause(&self, d: Duration) {
        std::thread::sleep(d);
    }

    /// A timed condition wait; a poisoned lock is used as it is.
    fn wait<'a, T>(&self, cv: &Condvar, guard: MutexGuard<'a, T>, d: Duration) -> MutexGuard<'a, T> {
        cv.wait_timeout(guard, d).unwrap_or_else(|e| e.into_inner()).0
    }
}
