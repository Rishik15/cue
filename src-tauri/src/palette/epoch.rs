//! File: palette_epoch.rs
//! Purpose: Which palette request is still wanted. A palette session number only ever grows (every open and every close takes a new one), and requests within a session carry a sequence number, so a request from a closed palette can never be revived by a reopen and a request that waited behind a newer one is skipped.
//! Contents: Epoch — open / cancel / admit / wanted; the process-wide `EPOCH` the palette uses.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

/// The session counter and the newest request seen in the current session.
pub struct Epoch {
    session: AtomicU64,
    newest: Mutex<(u64, u64)>,
}

impl Epoch {
    /// A fresh epoch: session 0 is never handed out, so a request that carries 0 is never wanted.
    pub const fn new() -> Epoch {
        Epoch { session: AtomicU64::new(0), newest: Mutex::new((0, 0)) }
    }

    /// A palette opened: the new session number. Every earlier session is over.
    pub fn open(&self) -> u64 {
        self.session.fetch_add(1, Ordering::AcqRel) + 1
    }

    /// The palette closed: every running or waiting request is stale, and the next open takes a later number (the counter is never reset).
    pub fn cancel(&self) {
        self.session.fetch_add(1, Ordering::AcqRel);
    }

    /// Takes in request `seq` of `session`. False when the session is over, or a newer request of it was already seen (this one waited behind it).
    pub fn admit(&self, session: u64, seq: u64) -> bool {
        let mut newest = self.newest.lock().unwrap_or_else(|e| e.into_inner());
        if session != self.session.load(Ordering::Acquire) {
            return false;
        }
        if newest.0 != session {
            *newest = (session, seq);
            return true;
        }
        if seq < newest.1 {
            return false;
        }
        newest.1 = seq;
        true
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
}
