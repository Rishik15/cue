//! File: palette_epoch.rs
//! Purpose: Which palette request is still wanted. A palette session number only ever grows (every open and every close takes a new one), and requests within a session carry a sequence number, so a request from a closed palette can never be revived by a reopen and a request that waited behind a newer one is skipped.
//! Contents: Epoch — open / cancel / admit / wanted / enter_lane (one request works at a time, a request that is no longer wanted leaves the queue at once); Lane — the right to work; the process-wide `EPOCH` the palette uses.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Condvar, Mutex};

/// The session counter and the newest request seen in the current session.
pub struct Epoch {
    session: AtomicU64,
    newest: Mutex<(u64, u64)>,
    lane: Mutex<bool>,
    lane_free: Condvar,
}

/// The right to run: only one request works at a time, the others wait for it in `enter_lane`. Dropping it lets the next in.
pub struct Lane<'a>(&'a Epoch);

impl Drop for Lane<'_> {
    /// Frees the lane and wakes the waiting requests.
    fn drop(&mut self) {
        *self.0.lane.lock().unwrap_or_else(|e| e.into_inner()) = false;
        self.0.lane_free.notify_all();
    }
}

impl Epoch {
    /// A fresh epoch: session 0 is never handed out, so a request that carries 0 is never wanted.
    pub const fn new() -> Epoch {
        Epoch { session: AtomicU64::new(0), newest: Mutex::new((0, 0)), lane: Mutex::new(false), lane_free: Condvar::new() }
    }

    /// A palette opened: the new session number. Every earlier session is over.
    pub fn open(&self) -> u64 {
        let session = self.session.fetch_add(1, Ordering::AcqRel) + 1;
        self.wake();
        session
    }

    /// The palette closed: every running or waiting request is stale, and the next open takes a later number (the counter is never reset).
    pub fn cancel(&self) {
        self.session.fetch_add(1, Ordering::AcqRel);
        self.wake();
    }

    /// Takes in request `seq` of `session`. False when the session is over, or a newer request of it was already seen (this one waited behind it).
    pub fn admit(&self, session: u64, seq: u64) -> bool {
        let mut newest = self.newest.lock().unwrap_or_else(|e| e.into_inner());
        if session != self.session.load(Ordering::Acquire) {
            return false;
        }
        if newest.0 != session {
            *newest = (session, seq);
            drop(newest);
            self.wake();
            return true;
        }
        if seq < newest.1 {
            return false;
        }
        newest.1 = seq;
        drop(newest);
        self.wake();
        true
    }

    /// Wakes the requests waiting in `enter_lane` so they look again whether they are still wanted. The lane lock is taken first so a waiter between its check and its wait cannot miss the wake.
    fn wake(&self) {
        drop(self.lane.lock().unwrap_or_else(|e| e.into_inner()));
        self.lane_free.notify_all();
    }

    /// Waits for the one working place (the text model takes one request at a time) and returns it, or None the moment the request is no longer wanted: its palette closed, or a newer request came in. A request that is no longer wanted therefore never stays blocked behind a stalled one, and nothing polls.
    pub fn enter_lane(&self, session: u64, seq: u64) -> Option<Lane<'_>> {
        let mut busy = self.lane.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            if !self.wanted(session, seq) {
                return None;
            }
            if !*busy {
                *busy = true;
                return Some(Lane(self));
            }
            busy = self.lane_free.wait(busy).unwrap_or_else(|e| e.into_inner());
        }
    }

    /// Whether request `seq` of `session` is still wanted: its session is current and no newer request of it came in.
    pub fn wanted(&self, session: u64, seq: u64) -> bool {
        let newest = self.newest.lock().unwrap_or_else(|e| e.into_inner());
        session == self.session.load(Ordering::Acquire) && (newest.0 != session || newest.1 <= seq)
    }
}

/// The palette's epoch.
pub static EPOCH: Epoch = Epoch::new();

#[cfg(test)]
mod tests {
    use super::Epoch;

    /// A request that waited behind a newer one of its session is skipped; the newer one runs.
    #[test]
    fn an_older_request_behind_a_newer_one_is_skipped() {
        let epoch = Epoch::new();
        let session = epoch.open();
        assert!(epoch.admit(session, 1));
        assert!(epoch.admit(session, 3));
        assert!(!epoch.admit(session, 2));
        assert!(!epoch.wanted(session, 1));
        assert!(epoch.wanted(session, 3));
    }

    /// Closing and reopening never revives an old request, however large its sequence number: the session number only grows.
    #[test]
    fn a_closed_session_stays_dead_after_a_reopen() {
        let epoch = Epoch::new();
        let old = epoch.open();
        assert!(epoch.admit(old, 40));
        epoch.cancel();
        let new = epoch.open();
        assert!(new > old);
        assert!(!epoch.admit(old, 41));
        assert!(!epoch.wanted(old, 41));
        assert!(epoch.admit(new, 1));
        assert!(epoch.wanted(new, 1));
        assert!(!epoch.wanted(old, 40));
    }

    /// A request that is running when the palette closes is no longer wanted when it finishes, and a request carrying session 0 never is.
    #[test]
    fn a_running_request_is_stale_once_the_palette_closes() {
        let epoch = Epoch::new();
        let session = epoch.open();
        assert!(epoch.admit(session, 1));
        epoch.cancel();
        assert!(!epoch.wanted(session, 1));
        assert!(!epoch.admit(0, 1));
    }

    /// A request waiting for the lane leaves as soon as a newer request arrives or the palette closes, even while the request in front of it never finishes; a free lane is taken at once.
    #[test]
    fn a_waiting_request_leaves_when_it_is_superseded() {
        let epoch = std::sync::Arc::new(Epoch::new());
        let session = epoch.open();
        assert!(epoch.admit(session, 1));
        let running = epoch.enter_lane(session, 1).expect("the free lane is taken");
        assert!(epoch.admit(session, 2));
        let waiter = {
            let epoch = epoch.clone();
            std::thread::spawn(move || epoch.enter_lane(session, 2).is_none())
        };
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert!(epoch.admit(session, 3));
        assert!(waiter.join().unwrap(), "the older waiter gave up");
        let second = {
            let epoch = epoch.clone();
            std::thread::spawn(move || epoch.enter_lane(session, 3).is_none())
        };
        std::thread::sleep(std::time::Duration::from_millis(50));
        epoch.cancel();
        assert!(second.join().unwrap(), "closing the palette released the waiter");
        drop(running);
    }
}
