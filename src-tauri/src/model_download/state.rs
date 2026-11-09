//! Purpose: The book-keeping of model downloads, with no I/O: which downloads run, the generation of each, file locks, delete reservations and disk reservations.
//! Contents: Phase — what a running download is doing; Last — how the previous download of a model ended; Ctl — the handle one download is stopped through; Manager — admit / finish / cancel / lock / reserve_delete / view / shutdown, all under one mutex that is never held while waiting on I/O.
//! Disk room is accounted by `ledger` (per path, under the registry lock).
//! Every admitted download runs at once (no queue, no cap); only a repeat of the same model, a delete in progress or a full disk refuses a start.

use std::collections::{BTreeMap, BTreeSet};

use super::ledger::{step, Claim, JobKey, Ledger, Op};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock, PoisonError};
use std::time::Duration;

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

/// How the last download of a model ended; kept until that model starts again or is deleted, so a card can still say what happened.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Last {
    pub generation: u64,
    pub outcome: &'static str,
    pub error: Option<String>,
}

/// The key of a file in the disk books and the file locks: its path, lower case because Windows paths ignore case.
pub fn path_key(dir: &std::path::Path, name: &str) -> String {
    dir.join(name).to_string_lossy().to_lowercase()
}

/// What a phase registers to be woken from a blocking call when the download is cancelled: closing a socket, killing a child process.
pub type Interrupt = Box<dyn FnOnce() + Send>;

/// The cancel object of one download: a flag every loop checks, and the interrupts each phase registered (the socket in flight, the unpacker child), run once when the download is cancelled so a blocked call returns at once.
pub struct Ctl {
    cancelled: AtomicBool,
    interrupts: Mutex<Vec<(u64, Interrupt)>>,
    next_interrupt: AtomicU64,
    /// Bytes this download has written so far (progress).
    pub done: AtomicU64,
    books: &'static Manager,
    job: JobKey,
    phase: AtomicU8,
    /// Called when the phase changes, so pages hear about it.
    on_phase: OnceLock<Box<dyn Fn() + Send + Sync>>,
}

impl Ctl {
    /// A fresh handle: not cancelled, nothing written.
    pub(super) fn new(books: &'static Manager, job: JobKey) -> Arc<Ctl> {
        Arc::new(Ctl {
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

    /// Sets the flag and runs every registered interrupt.
    fn stop(&self) {
        let pending = {
            let mut list = self.interrupts.lock().unwrap_or_else(PoisonError::into_inner);
            self.cancelled.store(true, Ordering::Release);
            std::mem::take(&mut *list)
        };
        pending.into_iter().for_each(|(_, interrupt)| interrupt());
    }
}

struct Job {
    generation: u64,
    group: &'static str,
    ctl: Arc<Ctl>,
    total: u64,
}

/// One running download as the pages see it.
pub struct JobView {
    pub phase: Phase,
    pub generation: u64,
    pub done: u64,
    pub total: u64,
}

/// Everything a page needs to know about downloads at one moment.
pub struct View {
    pub revision: u64,
    jobs: BTreeMap<&'static str, JobView>,
    last: BTreeMap<&'static str, Last>,
    text_busy: bool,
}

impl View {
    /// How the previous download of model `id` ended, if it is still remembered.
    pub fn last(&self, id: &str) -> Option<&Last> {
        self.last.get(id)
    }

    /// The running download of model `id`, if any.
    pub fn job(&self, id: &str) -> Option<&JobView> {
        self.jobs.get(id)
    }

    /// Whether Delete must be refused for `id`: it downloads, or it belongs to a group whose files running downloads share.
    pub fn delete_blocked(&self, id: &str, group: &str) -> bool {
        self.jobs.contains_key(id) || (shares_files(group) && self.text_busy)
    }
}

/// The text models share one server folder, so removing a text model must not run while another text download writes into it.
fn shares_files(group: &str) -> bool {
    group == "text"
}

struct Registry {
    jobs: BTreeMap<&'static str, Job>,
    ledger: Ledger,
    last: BTreeMap<&'static str, Last>,
    deleting: Vec<(&'static str, &'static str)>,
    locks: BTreeSet<String>,
    revision: u64,
    next: u64,
    closing: bool,
}

/// The one table of downloads. A `static` in production; tests make their own.
pub struct Manager {
    state: Mutex<Registry>,
    wake: Condvar,
}

/// The sizes of a download: all its files, and what it still has to write per path (fetching and unpacking, with headroom).
pub struct Sizes {
    pub total: u64,
    pub claims: Vec<Claim>,
}

/// A started download: its generation (so stale events and cancels can be told apart) and the revision after it was added.
pub struct Admitted {
    pub generation: u64,
    pub revision: u64,
    pub ctl: Arc<Ctl>,
}

/// Holds one file path for the duration of its use; others wanting the same path wait.
pub struct PathLock {
    manager: &'static Manager,
    key: String,
}

impl Drop for PathLock {
    /// Frees the path and wakes everyone waiting for it.
    fn drop(&mut self) {
        self.manager.registry().locks.remove(&self.key);
        self.manager.wake.notify_all();
    }
}

/// Holds a delete reservation until dropped.
pub struct DeleteGuard {
    manager: &'static Manager,
    id: &'static str,
}

impl Drop for DeleteGuard {
    /// Ends the reservation; the revision moves so pages notice.
    fn drop(&mut self) {
        let mut reg = self.manager.registry();
        reg.deleting.retain(|(id, _)| *id != self.id);
        reg.revision += 1;
    }
}

impl Manager {
    /// An empty table of downloads.
    pub const fn new() -> Manager {
        Manager {
            state: Mutex::new(Registry {
                jobs: BTreeMap::new(),
                ledger: Ledger::new(),
                last: BTreeMap::new(),
                deleting: Vec::new(),
                locks: BTreeSet::new(),
                revision: 0,
                next: 0,
                closing: false,
            }),
            wake: Condvar::new(),
        }
    }

    /// The table, locked; a panic elsewhere must not lock everyone out.
    fn registry(&self) -> MutexGuard<'_, Registry> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Adds a download unless `id` already runs, a delete holds it (or its shared files), or its claims do not fit next to what running downloads still have to write. `free` reads the room on the drive; it is called under the lock so the number is current next to the debt.
    pub fn admit(&'static self, id: &'static str, group: &'static str, sizes: Sizes, free: &dyn Fn() -> u64) -> Result<Admitted, String> {
        let Sizes { total, claims } = sizes;
        let mut reg = self.registry();
        if reg.closing {
            return Err("Cue is closing.".into());
        }
        if reg.jobs.contains_key(id) {
            return Err("This model is already downloading.".into());
        }
        if reg.deleting.iter().any(|(d, g)| *d == id || (shares_files(group) && *g == group)) {
            return Err("This model is being deleted. Try again in a moment.".into());
        }
        let generation = reg.next + 1;
        let job = (id, generation);
        let room = free();
        step(&mut reg.ledger, Op::Admit { job, claims: &claims, free: room })
            .map_err(|short| format!("Not enough free disk space: this download needs about {} MB more.", short.0.div_ceil(1_000_000)))?;
        reg.next = generation;
        reg.revision += 1;
        reg.last.remove(id);
        let (revision, ctl) = (reg.revision, Ctl::new(self, job));
        reg.jobs.insert(id, Job { generation, group, ctl: ctl.clone(), total });
        Ok(Admitted { generation, revision, ctl })
    }

    /// Applies one change to the books; a refusal is only possible for the ops that check room, which callers of this method do not use.
    fn account(&self, op: Op<'_>) {
        let _ = step(&mut self.registry().ledger, op);
    }

    /// Re-plans the download `job` to exactly `claims`, refused when the drive cannot hold the extra next to the other downloads.
    fn retarget(&self, job: JobKey, claims: &[Claim], free: &dyn Fn() -> u64) -> Result<(), String> {
        let mut reg = self.registry();
        step(&mut reg.ledger, Op::Retarget { job, claims, free: free() })
            .map_err(|_| "Not enough free disk space for this download.".to_string())
    }

    /// Removes the download of `id` if it is still generation `generation`; returns the new revision.
    pub fn finish(&self, id: &str, generation: u64) -> u64 {
        let mut reg = self.registry();
        if reg.jobs.get(id).is_some_and(|j| j.generation == generation) {
            if let Some(job) = reg.jobs.remove(id) {
                let _ = step(&mut reg.ledger, Op::Release { job: job.ctl.job });
            }
            reg.revision += 1;
        }
        let revision = reg.revision;
        drop(reg);
        self.wake.notify_all();
        revision
    }

    /// Like `finish`, and remembers how the download ended (`outcome`, `error`) until the model starts again or is deleted. A download cancelled by the time it concludes can never publish success, so "succeeded" becomes "cancelled". A stale generation changes nothing. Returns the new revision and the outcome recorded.
    pub fn conclude(&self, id: &'static str, generation: u64, outcome: &'static str, error: Option<String>) -> (u64, &'static str) {
        let mut reg = self.registry();
        let mut outcome = outcome;
        if let Some(job) = reg.jobs.get(id).filter(|j| j.generation == generation) {
            if outcome == "succeeded" && job.ctl.is_cancelled() {
                outcome = "cancelled";
            }
            reg.last.insert(id, Last { generation, outcome, error });
        }
        drop(reg);
        (self.finish(id, generation), outcome)
    }

    /// Moves the revision on without other change (a phase changed); returns it.
    pub fn touch(&self) -> u64 {
        let mut reg = self.registry();
        reg.revision += 1;
        reg.revision
    }

    /// Stops the download of `id` when it is still generation `generation`; a stale generation changes nothing.
    pub fn cancel(&self, id: &str, generation: u64) {
        let reg = self.registry();
        let ctl = reg.jobs.get(id).filter(|j| j.generation == generation).map(|j| j.ctl.clone());
        drop(reg);
        if let Some(ctl) = ctl {
            ctl.stop();
            let _sync = self.registry();
            self.wake.notify_all();
        }
    }

    /// Waits for the exclusive use of `key`, a file path shared between models. `Err(())` when `ctl` was cancelled while waiting.
    pub fn lock(&'static self, key: String, ctl: &Ctl) -> Result<PathLock, ()> {
        let mut reg = self.registry();
        loop {
            if ctl.is_cancelled() {
                return Err(());
            }
            if reg.locks.insert(key.clone()) {
                drop(reg);
                return Ok(PathLock { manager: self, key });
            }
            reg = self.wake.wait(reg).unwrap_or_else(PoisonError::into_inner);
        }
    }

    /// Reserves `id` for deleting. Refused while it downloads, or (text models) while any text model downloads.
    pub fn reserve_delete(&'static self, id: &'static str, group: &'static str) -> Result<DeleteGuard, String> {
        let mut reg = self.registry();
        let busy = reg.jobs.contains_key(id) || (shares_files(group) && reg.jobs.values().any(|j| j.group == group));
        if busy || reg.deleting.iter().any(|(d, _)| *d == id) {
            return Err(if reg.jobs.contains_key(id) {
                "Cancel the download first."
            } else {
                "Wait for the other download or delete to finish."
            }
            .into());
        }
        reg.deleting.push((id, group));
        reg.last.remove(id);
        reg.revision += 1;
        Ok(DeleteGuard { manager: self, id })
    }

    /// The running downloads and the revision, read together.
    pub fn view(&self) -> View {
        let reg = self.registry();
        let jobs = reg
            .jobs
            .iter()
            .map(|(id, j)| {
                (*id, JobView { phase: j.ctl.phase(), generation: j.generation, done: j.ctl.done.load(Ordering::Relaxed), total: j.total })
            })
            .collect();
        View { revision: reg.revision, jobs, last: reg.last.clone(), text_busy: reg.jobs.values().any(|j| shares_files(j.group)) }
    }

    /// Stops every download and waits up to `limit` for their threads to leave. Partial files stay.
    pub fn shutdown(&self, limit: Duration) {
        let ctls: Vec<_> = {
            let mut reg = self.registry();
            reg.closing = true;
            reg.jobs.values().map(|j| j.ctl.clone()).collect()
        };
        ctls.iter().for_each(|c| c.stop());
        let mut reg = self.registry();
        self.wake.notify_all();
        let deadline = std::time::Instant::now() + limit;
        while !reg.jobs.is_empty() {
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            if left.is_zero() {
                return;
            }
            reg = self.wake.wait_timeout(reg, left).unwrap_or_else(PoisonError::into_inner).0;
        }
    }
}
