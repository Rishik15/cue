//! File: slot.rs
//! Purpose: The handle to a worker thread that exists only while there is work: senders start it on demand, restart it when it died, and
//! the thread retires itself when idle. Start, send and retire all decide under one lock, so a message sent while the thread is deciding
//! to leave is never lost.
//! Contents: Slot — send (start if needed), try_send (never starts), retire (the thread's last act).

use std::io;
use std::sync::mpsc::{self, Receiver, SendError, Sender};
use std::sync::{Mutex, MutexGuard, PoisonError};

/// The sender of a thread that may not be running. `M` is the thread's message type.
pub struct Slot<M>(Mutex<Option<Sender<M>>>);

impl<M> Slot<M> {
    /// An empty slot: no thread yet.
    pub const fn new() -> Self {
        Slot(Mutex::new(None))
    }

    /// The lock; a panic elsewhere must not make the slot unusable for good.
    fn lock(&self) -> MutexGuard<'_, Option<Sender<M>>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Delivers `msg`, starting the thread with `spawn` when there is none or the old one is gone. Inputs: the message and a starter that
    /// receives a sender for the thread's own use and the receiver it must read. Returns Ok once the message is queued, otherwise the message
    /// back with the reason the thread could not start.
    pub fn send(&self, mut msg: M, spawn: impl FnOnce(Sender<M>, Receiver<M>) -> io::Result<()>) -> Result<(), (M, io::Error)> {
        let mut slot = self.lock();
        if let Some(tx) = slot.as_ref() {
            match tx.send(msg) {
                Ok(()) => return Ok(()),
                Err(SendError(back)) => msg = back,
            }
        }
        let (tx, rx) = mpsc::channel();
        *slot = None;
        if let Err(why) = spawn(tx.clone(), rx) {
            return Err((msg, why));
        }
        match tx.send(msg) {
            Ok(()) => {
                *slot = Some(tx);
                Ok(())
            }
            Err(SendError(back)) => Err((back, io::Error::other("the thread stopped as soon as it started"))),
        }
    }

    /// Delivers `msg` only to a running thread. Returns the message back when there is none (it retired or never started).
    pub fn try_send(&self, msg: M) -> Result<(), M> {
        let mut slot = self.lock();
        let Some(tx) = slot.as_ref() else { return Err(msg) };
        match tx.send(msg) {
            Ok(()) => Ok(()),
            Err(SendError(back)) => {
                *slot = None;
                Err(back)
            }
        }
    }

    /// The thread's last act when it has nothing to do. Returns a message that raced in (the thread must handle it and carry on), or None
    /// after the slot was emptied under the lock: the thread must return now, and the next sender starts a new one.
    pub fn retire(&self, rx: &Receiver<M>) -> Option<M> {
        let mut slot = self.lock();
        match rx.try_recv() {
            Ok(msg) => Some(msg),
            Err(_) => {
                *slot = None;
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    use super::*;

    /// A first start that fails is reported with the message handed back, and the next send tries again.
    #[test]
    fn a_failed_start_is_not_cached() {
        let slot = Slot::<u32>::new();
        let refused = slot.send(1, |_, _| Err(io::Error::other("no threads")));
        assert!(matches!(refused, Err((1, _))));
        assert_eq!(slot.try_send(2), Err(2), "nothing runs");
        let kept = Arc::new(Mutex::new(None));
        let hold = kept.clone();
        assert!(slot
            .send(3, move |_, rx| {
                *hold.lock().unwrap() = Some(rx);
                Ok(())
            })
            .is_ok());
        assert_eq!(kept.lock().unwrap().as_ref().and_then(|r| r.try_recv().ok()), Some(3));
    }

    /// A thread that died (its receiver dropped) is replaced, and a try_send to it gives the message back instead of losing it.
    #[test]
    fn a_dead_thread_is_replaced_and_try_send_returns_the_message() {
        let slot = Slot::<u32>::new();
        let died = slot.send(1, |_, rx| {
            drop(rx);
            Ok(())
        });
        assert!(matches!(died, Err((1, _))), "a thread that is gone at once is an error with the message back");
        assert_eq!(slot.try_send(2), Err(2));
        let mut keep = None;
        assert!(slot
            .send(5, |_, rx| {
                keep = Some(rx);
                Ok(())
            })
            .is_ok());
        assert_eq!(keep.and_then(|r| r.try_recv().ok()), Some(5));
    }

    /// Senders race a thread that retires whenever it is idle: every message is handled exactly once, whichever side wins.
    #[test]
    fn a_start_racing_a_retirement_loses_nothing() {
        const SENDERS: usize = 4;
        const EACH: usize = 500;
        let slot = Arc::new(Slot::<usize>::new());
        let handled = Arc::new(AtomicUsize::new(0));
        let spawn = |slot: Arc<Slot<usize>>, handled: Arc<AtomicUsize>| {
            move |_tx: Sender<usize>, rx: Receiver<usize>| {
                thread::spawn(move || {
                    let mut next = slot.retire(&rx);
                    while let Some(_msg) = next {
                        handled.fetch_add(1, Ordering::Relaxed);
                        next = match rx.try_recv() {
                            Ok(msg) => Some(msg),
                            Err(_) => slot.retire(&rx),
                        };
                    }
                });
                Ok(())
            }
        };
        let senders: Vec<_> = (0..SENDERS)
            .map(|_| {
                let (slot, handled) = (slot.clone(), handled.clone());
                thread::spawn(move || {
                    for n in 0..EACH {
                        let starter = spawn(slot.clone(), handled.clone());
                        assert!(slot.send(n, starter).is_ok());
                        if n % 7 == 0 {
                            thread::yield_now();
                        }
                    }
                })
            })
            .collect();
        for sender in senders {
            sender.join().expect("sender");
        }
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while handled.load(Ordering::Relaxed) < SENDERS * EACH && std::time::Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(handled.load(Ordering::Relaxed), SENDERS * EACH);
    }
}
