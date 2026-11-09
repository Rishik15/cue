//! File: model_download/ctl.rs
//! Purpose: The handle of one running download: its cancel flag and interrupts, its commit point, its phase, and the disk-book calls it makes. The table that owns it is `state::Manager`.
//! Contents: Phase — what a download is doing; Interrupt — what a blocking call registers; Ctl — the handle.
//! A cancel sets the flag and runs every registered interrupt once. Once the download is committed (it started publishing an install) a cancel is ignored, so a published model is never reported as cancelled.

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError, Weak};

use super::ledger::{Claim, JobKey, Op};
use super::state::Manager;

/// What a running download is doing, as the card shows it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    Fetching,
    Verifying,
    Installing,
}

impl Phase {
    /// The name the pages use.
    pub fn name(self) -> &'static str {
        match self {
            Phase::Fetching => "fetching",
            Phase::Verifying => "verifying",
            Phase::Installing => "installing",
        }
    }

    /// The phase stored as `code`; fetching for anything unknown.
    fn from_code(code: u8) -> Phase {
        match code {
            1 => Phase::Verifying,
            2 => Phase::Installing,
            _ => Phase::Fetching,
        }
    }
}

/// What a phase registers to be woken from a blocking call when the download is cancelled: closing a socket, killing a child process.
pub type Interrupt = Box<dyn FnOnce() + Send>;

/// The cancel object of one download: a flag every loop checks, and the interrupts each phase registered (the socket in flight, the unpacker child), run once when the download is cancelled so a blocked call returns at once.
pub struct Ctl {
    /// Called when the phase changes, so pages hear about it.
    cancelled: AtomicBool,
    interrupts: Mutex<Vec<(u64, Interrupt)>>,
    next_interrupt: AtomicU64,
    /// Bytes this download has written so far (progress).
    pub done: AtomicU64,
    books: &'static Manager,
    job: JobKey,
    me: Weak<Ctl>,
    phase: AtomicU8,
    on_phase: OnceLock<Box<dyn Fn() + Send + Sync>>,
}

impl Ctl {
    /// A fresh handle: not cancelled, nothing written.
    pub(super) fn new(books: &'static Manager, job: JobKey) -> Arc<Ctl> {
        Arc::new_cyclic(|me| Ctl {
            me: me.clone(),
            cancelled: AtomicBool::new(false),
            interrupts: Mutex::new(Vec::new()),
            next_interrupt: AtomicU64::new(1),
            done: AtomicU64::new(0),
            books,
            job,
            phase: AtomicU8::new(0),
            on_phase: OnceLock::new(),
        })
    }

    /// Sets what to call when the phase changes; only the first call counts.
    pub fn watch_phase(&self, notify: Box<dyn Fn() + Send + Sync>) {
        let _ = self.on_phase.set(notify);
    }

    /// Moves to `phase`; tells the watcher only when it differs from the current one.
    pub fn set_phase(&self, phase: Phase) {
        if self.phase.swap(phase as u8, Ordering::AcqRel) != phase as u8 {
            if let Some(notify) = self.on_phase.get() {
                notify();
            }
        }
    }

    /// What the download is doing now.
    pub fn phase(&self) -> Phase {
        Phase::from_code(self.phase.load(Ordering::Acquire))
    }

    /// Notes, before the delete, that `bytes` already on disk for `key` are thrown away and have to be written again.
    pub fn lose(&self, key: &str, bytes: u64) {
        self.books.account(Op::Lose { key, bytes });
    }

    /// Notes that `bytes` more were written to `key`.
    pub fn wrote(&self, key: &str, bytes: u64) {
        self.books.account(Op::Wrote { key, bytes });
    }

    /// Plans again from the disk as it is now: the download needs exactly `claims`. Refused when the drive cannot hold what they add to the other downloads' debt.
    pub fn retarget(&self, claims: &[Claim], free: &dyn Fn() -> u64) -> Result<(), String> {
        self.books.retarget(self.job, claims, free)
    }

    /// Whether a cancel was requested.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    /// Registers `interrupt` to run when the download is cancelled; runs it at once (and returns 0) when it was cancelled already. Returns the id for `unregister`.
    pub fn register(&self, interrupt: Interrupt) -> u64 {
        let mut list = self.interrupts.lock().unwrap_or_else(PoisonError::into_inner);
        if self.is_cancelled() {
            drop(list);
            interrupt();
            return 0;
        }
        let id = self.next_interrupt.fetch_add(1, Ordering::Relaxed);
        list.push((id, interrupt));
        id
    }

    /// Drops an interrupt whose blocking call is over, so a later cancel cannot act on something finished.
    pub fn unregister(&self, id: u64) {
        self.interrupts.lock().unwrap_or_else(PoisonError::into_inner).retain(|(i, _)| *i != id);
    }

    /// Sets the flag and takes the registered interrupts; the caller runs them with `run_interrupts` once it has left the table's lock.
    pub(super) fn mark_cancelled(&self) -> Vec<(u64, Interrupt)> {
        let mut list = self.interrupts.lock().unwrap_or_else(PoisonError::into_inner);
        self.cancelled.store(true, Ordering::Release);
        std::mem::take(&mut *list)
    }

    /// Runs interrupts taken by `mark_cancelled`.
    pub(super) fn run_interrupts(pending: Vec<(u64, Interrupt)>) {
        pending.into_iter().for_each(|(_, interrupt)| interrupt());
    }

    /// The commit point: asks to start publishing the install. True when the download is now committed (a later cancel is ignored); false when a cancel came first, and the caller must stop and clean up.
    pub fn commit(&self) -> bool {
        self.books.commit(self.job)
    }

    /// Sets the total bytes the pages show for this download, after a re-plan changed it.
    pub fn set_total(&self, total: u64) {
        self.books.set_total(self.job, total);
    }

    /// The key of this download in the books.
    pub(super) fn job(&self) -> JobKey {
        self.job
    }

    /// This handle as an `Arc`, for something that outlives the call it was passed to (a body reader). None only while the handle is being dropped.
    pub fn shared(&self) -> Option<Arc<Ctl>> {
        self.me.upgrade()
    }
}

impl Ctl {
    /// A handle on a table of its own with one admitted download, for tests of code that only needs the cancel object.
    #[cfg(test)]
    pub fn detached() -> Arc<Ctl> {
        use super::state::Sizes;
        let books: &'static Manager = Box::leak(Box::new(Manager::new()));
        books.admit("test", "speech", Sizes { total: 0, claims: vec![] }, &|| u64::MAX).expect("admitted").ctl
    }

    /// How many interrupts are registered, for tests of cleanup.
    #[cfg(test)]
    pub fn interrupt_count(&self) -> usize {
        self.interrupts.lock().unwrap_or_else(PoisonError::into_inner).len()
    }

    /// Cancels this handle the way the table does, for tests.
    #[cfg(test)]
    pub fn cancel_for_test(&self) {
        self.books.cancel(self.job.0, self.job.1);
    }
}
