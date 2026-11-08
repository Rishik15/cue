//! Purpose: The book-keeping of model downloads, with no I/O: which downloads run, the generation of each, file locks, delete reservations and disk reservations.
//! Contents: Ctl — the handle one download is stopped through; Manager — admit / finish / cancel / lock / reserve_delete / view / shutdown, all under one mutex that is never held while waiting on I/O.
//! Every admitted download runs at once (no queue, no cap); only a repeat of the same model, a delete in progress or a full disk refuses a start.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

/// What stops one download: a flag every loop checks, and the HTTP agent of the attempt in flight, shut down so a blocked read returns at once.
pub struct Ctl {
    cancelled: AtomicBool,
    agent: Mutex<Option<ureq::Agent>>,
    /// Bytes this download has written so far (progress and the share of its disk reservation already used).
    pub done: AtomicU64,
    /// Bytes this run has written to disk so far.
    written: AtomicU64,
    /// Bytes on disk the download threw away and has to write again (a server that ignored the resume range, a failed hash).
    lost: AtomicU64,
}

impl Ctl {
    /// A fresh handle: not cancelled, nothing written.
    pub(super) fn new() -> Arc<Ctl> {
        Arc::new(Ctl {
            cancelled: AtomicBool::new(false),
            agent: Mutex::new(None),
            done: AtomicU64::new(0),
            written: AtomicU64::new(0),
            lost: AtomicU64::new(0),
        })
    }

    /// Notes that `bytes` already on disk were thrown away, so the disk reservation covers writing them again.
    pub fn lose(&self, bytes: u64) {
        self.lost.fetch_add(bytes, Ordering::Relaxed);
    }

    /// Notes that `bytes` more were written to disk.
    pub fn wrote(&self, bytes: u64) {
        self.written.fetch_add(bytes, Ordering::Relaxed);
    }

    /// Whether a cancel was requested.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    /// Remembers the agent of the attempt in flight so `stop` can cut its connection; false when the download was cancelled already.
    pub fn register(&self, agent: &ureq::Agent) -> bool {
        *self.agent.lock().unwrap_or_else(PoisonError::into_inner) = Some(agent.clone());
        !self.is_cancelled()
    }

    /// Sets the flag and shuts the connection of the attempt in flight.
    fn stop(&self) {
        self.cancelled.store(true, Ordering::Release);
        if let Some(agent) = self.agent.lock().unwrap_or_else(PoisonError::into_inner).take() {
            agent.shutdown();
        }
    }
}

struct Job {
    generation: u64,
    group: &'static str,
    ctl: Arc<Ctl>,
    total: u64,
    need: u64,
}

impl Job {
    /// Bytes this download still has to write: what it needed, less what it wrote, plus what it had to write again.
    fn owed(&self) -> u64 {
        (self.need + self.ctl.lost.load(Ordering::Relaxed)).saturating_sub(self.ctl.written.load(Ordering::Relaxed))
    }
}

/// One running download as the pages see it.
pub struct JobView {
    pub generation: u64,
    pub done: u64,
    pub total: u64,
}

/// Everything a page needs to know about downloads at one moment.
pub struct View {
    pub revision: u64,
    jobs: BTreeMap<&'static str, JobView>,
    text_busy: bool,
}

impl View {
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

/// The sizes of a download: all its files, and the bytes it still has to write (fetching and unpacking, with headroom).
pub struct Sizes {
    pub total: u64,
    pub need: u64,
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

    /// Adds a download unless `id` already runs, a delete holds it (or its shared files), or `need` bytes do not fit next to what running downloads still have to write. `free` reads the room on the drive; it is called under the lock so the number is current next to what running downloads still owe.
    pub fn admit(&self, id: &'static str, group: &'static str, sizes: Sizes, free: &dyn Fn() -> u64) -> Result<Admitted, String> {
        let Sizes { total, need } = sizes;
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
        let owed: u64 = reg.jobs.values().map(|j| j.owed()).sum();
        let free = free();
        if need.saturating_add(owed) > free {
            return Err(format!(
                "Not enough free disk space: this download needs about {} MB more.",
                (need + owed - free).div_ceil(1_000_000)
            ));
        }
        reg.next += 1;
        reg.revision += 1;
        let (generation, revision, ctl) = (reg.next, reg.revision, Ctl::new());
        reg.jobs.insert(id, Job { generation, group, ctl: ctl.clone(), total, need });
        Ok(Admitted { generation, revision, ctl })
    }

    /// Raises the reservation of a running download by `extra` bytes (it found it has more to write than planned) unless the drive cannot hold them next to the other downloads.
    pub fn grow(&self, id: &str, generation: u64, extra: u64, free: &dyn Fn() -> u64) -> Result<(), String> {
        let mut reg = self.registry();
        let owed: u64 = reg.jobs.values().map(Job::owed).sum();
        if extra.saturating_add(owed) > free() {
            return Err("Not enough free disk space for this download.".into());
        }
        if let Some(job) = reg.jobs.get_mut(id).filter(|j| j.generation == generation) {
            job.need += extra;
        }
        Ok(())
    }

    /// Removes the download of `id` if it is still generation `generation`; returns the new revision.
    pub fn finish(&self, id: &str, generation: u64) -> u64 {
        let mut reg = self.registry();
        if reg.jobs.get(id).is_some_and(|j| j.generation == generation) {
            reg.jobs.remove(id);
            reg.revision += 1;
        }
        let revision = reg.revision;
        drop(reg);
        self.wake.notify_all();
        revision
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
        reg.revision += 1;
        Ok(DeleteGuard { manager: self, id })
    }

    /// The running downloads and the revision, read together.
    pub fn view(&self) -> View {
        let reg = self.registry();
        let jobs = reg
            .jobs
            .iter()
            .map(|(id, j)| (*id, JobView { generation: j.generation, done: j.ctl.done.load(Ordering::Relaxed), total: j.total }))
            .collect();
        View { revision: reg.revision, jobs, text_busy: reg.jobs.values().any(|j| shares_files(j.group)) }
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
