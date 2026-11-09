//! File: model_download/state.rs
//! Purpose: The book-keeping of model downloads, with no I/O: which downloads run, the generation of each, file locks, delete reservations, commit points and disk reservations.
//! Contents: Outcome / Last — how a download ended and what is remembered of it; Manager — admit / commit / finish / conclude / cancel / lock / reserve_delete / view / shutdown, all under one mutex that is never held while waiting on I/O; View / JobView — what the pages read; Sizes / Admitted / PathLock / DeleteGuard.
//! The handle of one download is `ctl::Ctl`. Disk room is accounted by `ledger` (per path, under the registry lock).
//! Every admitted download runs at once (no queue, no cap); only a repeat of the same model, a delete in progress or a full disk refuses a start.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use serde::Serialize;

use super::ctl::{Ctl, Phase};
use super::ledger::{step, Claim, JobKey, Ledger, Op};

/// How a download ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Outcome {
    Succeeded,
    Cancelled,
    Failed,
}

/// How the last download of a model ended; kept until that model starts again or is deleted, so a card can still say what happened.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Last {
    pub generation: u64,
    pub outcome: Outcome,
    pub error: Option<String>,
}

/// A running download in the table.
struct Job {
    generation: u64,
    group: &'static str,
    ctl: Arc<Ctl>,
    total: u64,
    committed: bool,
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
        reg.jobs.insert(id, Job { generation, group, ctl: ctl.clone(), total, committed: false });
        Ok(Admitted { generation, revision, ctl })
    }

    /// Applies one change to the books; a refusal is only possible for the ops that check room, which callers of this method do not use.
    pub(super) fn account(&self, op: Op<'_>) {
        let _ = step(&mut self.registry().ledger, op);
    }

    /// Re-plans the download `job` to exactly `claims`, refused when the drive cannot hold the extra next to the other downloads.
    pub(super) fn retarget(&self, job: JobKey, claims: &[Claim], free: &dyn Fn() -> u64) -> Result<(), String> {
        let mut reg = self.registry();
        step(&mut reg.ledger, Op::Retarget { job, claims, free: free() })
            .map_err(|_| "Not enough free disk space for this download.".to_string())
    }

    /// Removes the download of `id` if it is still generation `generation`; returns the new revision.
    pub fn finish(&self, id: &str, generation: u64) -> u64 {
        let mut reg = self.registry();
        if reg.jobs.get(id).is_some_and(|j| j.generation == generation) {
            if let Some(job) = reg.jobs.remove(id) {
                let _ = step(&mut reg.ledger, Op::Release { job: job.ctl.job() });
            }
            reg.revision += 1;
        }
        let revision = reg.revision;
        drop(reg);
        self.wake.notify_all();
        revision
    }

    /// Like `finish`, and remembers how the download ended (`outcome`, `error`) until the model starts again or is deleted. A download cancelled by the time it concludes, and not committed, can never publish success, so `Succeeded` becomes `Cancelled`; a committed success stays. A stale generation changes nothing. Returns the new revision and the outcome recorded.
    pub fn conclude(&self, id: &'static str, generation: u64, outcome: Outcome, error: Option<String>) -> (u64, Outcome) {
        let mut reg = self.registry();
        let mut outcome = outcome;
        if let Some(job) = reg.jobs.get(id).filter(|j| j.generation == generation) {
            if outcome == Outcome::Succeeded && !job.committed && job.ctl.is_cancelled() {
                outcome = Outcome::Cancelled;
            }
            reg.last.insert(id, Last { generation, outcome, error });
        }
        drop(reg);
        (self.finish(id, generation), outcome)
    }

    /// The commit point of download `job`: true and a later cancel is ignored, or false when a cancel came first. Decided under the table's lock, so it cannot interleave with `cancel`.
    pub(super) fn commit(&self, job: JobKey) -> bool {
        let mut reg = self.registry();
        match reg.jobs.get_mut(job.0).filter(|j| j.generation == job.1) {
            Some(j) if !j.ctl.is_cancelled() => {
                j.committed = true;
                true
            }
            _ => false,
        }
    }

    /// Sets the total bytes shown for download `job`.
    pub(super) fn set_total(&self, job: JobKey, total: u64) {
        if let Some(j) = self.registry().jobs.get_mut(job.0).filter(|j| j.generation == job.1) {
            j.total = total;
        }
    }

    /// Moves the revision on without other change (a phase changed); returns it.
    pub fn touch(&self) -> u64 {
        let mut reg = self.registry();
        reg.revision += 1;
        reg.revision
    }

    /// Stops the download of `id` when it is still generation `generation` and not committed; a stale generation or a committed download changes nothing.
    pub fn cancel(&self, id: &str, generation: u64) {
        let pending = {
            let reg = self.registry();
            reg.jobs.get(id).filter(|j| j.generation == generation && !j.committed).map(|j| j.ctl.mark_cancelled())
        };
        if let Some(pending) = pending {
            Ctl::run_interrupts(pending);
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

    /// Stops every download that is not committed and waits up to `limit` for all threads to leave (a committed one is publishing and finishes at once). Partial files stay.
    pub fn shutdown(&self, limit: Duration) {
        let pending: Vec<_> = {
            let mut reg = self.registry();
            reg.closing = true;
            reg.jobs.values().filter(|j| !j.committed).map(|j| j.ctl.mark_cancelled()).collect()
        };
        pending.into_iter().for_each(Ctl::run_interrupts);
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
