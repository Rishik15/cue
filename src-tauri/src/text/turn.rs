//! File: text/turn.rs
//! Purpose: The one-at-a-time turn for text model requests: a waiter is woken the moment the holder gives the turn back, and leaves the queue when its cancel is set.
//! Contents: Turn — the turn; Turn::take — wait for it; Held — the held turn, given back on drop (a panic included).

use std::sync::{Condvar, Mutex};

use super::engine::POLL;
use super::Cancel;
use crate::lock::{lock, wait_timeout};

/// The turn: whether a request holds it, and the signal that it came free.
pub struct Turn {
    busy: Mutex<bool>,
    freed: Condvar,
}

/// A held turn; dropping it gives the turn back and wakes one waiter.
pub struct Held<'a>(&'a Turn);

impl Turn {
    /// A free turn. Returns it.
    pub const fn new() -> Self {
        Self { busy: Mutex::new(false), freed: Condvar::new() }
    }

    /// Waits for the turn, looking at `cancel` while it waits. Input: the caller's token. Returns the held turn, or None when the token was set before the turn came free; a free turn is taken whatever the token says.
    /// shortcut: `Cancel` is a predicate with no wake signal, so a waiter still looks at it every `POLL` (only while another request holds the turn); give `Cancel` a notifier if that ever costs.
    pub fn take(&self, cancel: &Cancel) -> Option<Held<'_>> {
        let mut busy = lock(&self.busy);
        while *busy {
            if cancel.is_set() {
                return None;
            }
            busy = wait_timeout(&self.freed, busy, POLL);
        }
        *busy = true;
        Some(Held(self))
    }
}

impl Drop for Held<'_> {
    /// Gives the turn back and wakes one waiter.
    fn drop(&mut self) {
        *lock(&self.0.busy) = false;
        self.0.freed.notify_one();
    }
}
