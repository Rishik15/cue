//! File: gate.rs
//! Purpose: The one-at-a-time gate of the clipboard coordinator: whoever holds it owns every Cue clipboard write and injected key until it is dropped, and a waiter gives up after a deadline instead of hanging.
//! Contents: Gate — the lock with a deadline; Held — the guard that frees it; Busy — the gate stayed taken.

use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

/// The gate stayed taken for the whole wait.
#[derive(Debug, PartialEq, Eq)]
pub struct Busy;

/// A lock that can be waited on for a bounded time (`std::sync::Mutex` has no timed lock).
pub struct Gate {
    held: Mutex<bool>,
    freed: Condvar,
}

/// Proof that the gate is held; dropping it frees the gate and wakes one waiter.
pub struct Held {
    gate: &'static Gate,
}

impl Gate {
    /// A free gate.
    pub const fn new() -> Gate {
        Gate { held: Mutex::new(false), freed: Condvar::new() }
    }

    /// Takes the gate, waiting at most `within`. Input: the longest wait. Returns the guard, or `Busy` when the gate stayed taken.
    pub fn enter(&'static self, within: Duration) -> Result<Held, Busy> {
        let deadline = Instant::now() + within;
        let mut held = self.held.lock().unwrap_or_else(|e| e.into_inner());
        while *held {
            let left = deadline.checked_duration_since(Instant::now()).filter(|d| !d.is_zero()).ok_or(Busy)?;
            held = self.freed.wait_timeout(held, left).unwrap_or_else(|e| e.into_inner()).0;
        }
        *held = true;
        Ok(Held { gate: self })
    }
}

impl Drop for Held {
    /// Frees the gate and wakes one waiter.
    fn drop(&mut self) {
        *self.gate.held.lock().unwrap_or_else(|e| e.into_inner()) = false;
        self.gate.freed.notify_one();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A taken gate refuses a second taker after the deadline, and takes it again once freed.
    #[test]
    fn a_taken_gate_is_busy_until_it_is_freed() {
        static GATE: Gate = Gate::new();
        let first = GATE.enter(Duration::from_millis(10)).expect("free gate");
        assert_eq!(GATE.enter(Duration::from_millis(20)).err(), Some(Busy));
        drop(first);
        assert!(GATE.enter(Duration::from_millis(10)).is_ok());
    }

    /// A waiter is woken by the release and does not wait for its whole deadline.
    #[test]
    fn a_waiter_gets_the_gate_when_it_is_freed() {
        static GATE: Gate = Gate::new();
        let first = GATE.enter(Duration::from_millis(10)).expect("free gate");
        let waiter = std::thread::spawn(|| GATE.enter(Duration::from_secs(5)).is_ok());
        std::thread::sleep(Duration::from_millis(30));
        drop(first);
        assert!(waiter.join().expect("waiter ran"));
    }
}
