//! File: text/turn.rs
//! Purpose: The one-at-a-time turn for text model requests: a waiter sleeps until the holder gives the turn back or a cancel source rings (`Cancel::ring`), and leaves the queue when its cancel is set. Nothing polls.
//! Contents: Turn — the turn; Turn::take — wait for it; Turn::wake — make every waiter look at its cancel again; Held — the held turn, given back on drop (a panic included).

use std::sync::{Condvar, Mutex};

use super::Cancel;
use crate::lock::{lock, wait};

/// The turn: whether a request holds it, and the signal that it came free or a cancel may have been set.
pub struct Turn {
    busy: Mutex<bool>,
    changed: Condvar,
}

/// A held turn; dropping it gives the turn back and wakes the waiters.
pub struct Held<'a>(&'a Turn);

impl Turn {
    /// A free turn. Returns it.
    pub const fn new() -> Self {
        Self { busy: Mutex::new(false), changed: Condvar::new() }
    }

    /// Waits for the turn, looking at `cancel` each time it is woken. Input: the caller's token. Returns the held turn, or None when the token was set before the turn came free; a free turn is taken whatever the token says.
    pub fn take(&self, cancel: &Cancel) -> Option<Held<'_>> {
        let mut busy = lock(&self.busy);
        while *busy {
            if cancel.is_set() {
                return None;
            }
            busy = wait(&self.changed, busy);
        }
        *busy = true;
        Some(Held(self))
    }

    /// Wakes every waiter so it looks at its cancel again. The turn's lock is taken first so a waiter between its check and its wait cannot miss the wake.
    pub fn wake(&self) {
        drop(lock(&self.busy));
        self.changed.notify_all();
    }
}

impl Drop for Held<'_> {
    /// Gives the turn back and wakes the waiters: all of them, since one woken waiter may be cancelled and leave without taking it.
    fn drop(&mut self) {
        *lock(&self.0.busy) = false;
        self.0.changed.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use super::{Cancel, Turn};

    /// A token over `flag` that counts how often it is asked in `asked`.
    fn counted(flag: &Arc<AtomicBool>, asked: &Arc<AtomicUsize>) -> Cancel {
        let (flag, asked) = (flag.clone(), asked.clone());
        Cancel::new(move || {
            asked.fetch_add(1, Ordering::SeqCst);
            flag.load(Ordering::SeqCst)
        })
    }

    /// A waiter does not wake while nothing happens, returns at once when its cancel is set and the turn is woken, and leaves the turn held.
    #[test]
    fn a_cancelled_waiter_returns_at_once_and_never_polls() {
        static TURN: Turn = Turn::new();
        let held = TURN.take(&Cancel::never()).unwrap();
        let (flag, asked) = (Arc::new(AtomicBool::new(false)), Arc::new(AtomicUsize::new(0)));
        let token = counted(&flag, &asked);
        let waiter = std::thread::spawn(move || TURN.take(&token).is_none());
        std::thread::sleep(Duration::from_millis(350));
        assert_eq!(asked.load(Ordering::SeqCst), 1, "the waiter looked at its cancel again with nothing changed");
        flag.store(true, Ordering::SeqCst);
        let rung = Instant::now();
        TURN.wake();
        assert!(waiter.join().unwrap(), "a cancelled waiter must not take the turn");
        assert!(rung.elapsed() < Duration::from_millis(50));
        drop(held);
        assert!(TURN.take(&Cancel::never()).is_some());
    }

    /// A release wakes a waiter that is not cancelled, and it takes the turn.
    #[test]
    fn a_release_hands_the_turn_on() {
        static TURN: Turn = Turn::new();
        let held = TURN.take(&Cancel::never()).unwrap();
        let waiter = std::thread::spawn(|| TURN.take(&Cancel::never()).is_some());
        std::thread::sleep(Duration::from_millis(20));
        drop(held);
        assert!(waiter.join().unwrap());
    }
}
