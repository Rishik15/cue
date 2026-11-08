//! File: input/save.rs
//! Purpose: Saving bindings in order. A candidate is validated against the pending-latest set (the stored bindings with every queued save applied), admitted to a bounded queue, written by one writer thread, and only a durable write activates it. A failed write rebases the set and re-checks what was queued behind it.
//! Contents: Store — where bindings live; validate — whole-set rules; Job / Pending — the ordered queue; Writer — the lazy writer thread.

use std::collections::VecDeque;
use std::sync::mpsc::{self, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};

use super::catalog::Spec;
use super::chord::Chord;
use super::error::InputError;

/// Where bindings and the on/off switches are kept.
pub trait Store: Send + Sync {
    /// The stored value of a shortcut, if any.
    fn load(&self, id: &str) -> Option<serde_json::Value>;
    /// Writes a shortcut's combination durably.
    fn save(&self, id: &str, chord: &Chord) -> Result<(), String>;
    /// Whether the feature and the shortcut are both on.
    fn enabled(&self, id: &str) -> bool;
}

const QUEUE: usize = 8;

/// One queued write: ticket, shortcut id, combination.
type Write = (u64, &'static str, Chord);

/// Called on the writer thread with a ticket and the result of its write.
pub type Done = Arc<dyn Fn(u64, Result<(), String>) + Send + Sync>;

/// Whether `chord` may become shortcut `idx`'s binding given every other binding (None for a row that is inactive).
pub fn validate(specs: &[Spec], others: &[Option<Chord>], idx: usize, chord: &Chord) -> Result<(), InputError> {
    if chord.reserved() {
        return Err(InputError::Reserved);
    }
    if !chord.usable() {
        return Err(InputError::InvalidBinding(
            "Add a modifier key (Ctrl, Alt, Shift, Win) or use an F-key. A modifier alone needs two, or Alt or Win.".into(),
        ));
    }
    let others = others.iter().enumerate().filter(|(i, _)| *i != idx).filter_map(|(i, c)| c.map(|c| (i, c)));
    for (i, other) in others {
        if other.intersects(chord) {
            return Err(InputError::Conflict(format!("Already used by {}.", specs[i].label)));
        }
        if other.nests(chord) {
            return Err(InputError::Conflict(format!("Overlaps {}.", specs[i].label)));
        }
    }
    Ok(())
}

/// One queued write; `reply` answers a reset or other caller that waits for the revision.
pub struct Job {
    pub ticket: u64,
    pub idx: usize,
    pub chord: Chord,
    pub lease: Option<u64>,
    pub reply: Option<mpsc::Sender<Result<u64, InputError>>>,
}

/// What became of a job after its write finished, for the caller to publish.
pub struct Settled {
    pub job: Job,
    pub result: Result<(), InputError>,
}

/// The stored bindings and the queue of writes not finished yet.
pub struct Pending {
    committed: Vec<Option<Chord>>,
    queue: VecDeque<Job>,
}

impl Pending {
    /// A set starting from `committed`.
    pub fn new(committed: Vec<Option<Chord>>) -> Pending {
        Pending { committed, queue: VecDeque::new() }
    }

    /// The bindings as they will be once every queued write lands.
    pub fn latest(&self) -> Vec<Option<Chord>> {
        let mut set = self.committed.clone();
        for job in &self.queue {
            set[job.idx] = Some(job.chord);
        }
        set
    }

    /// The oldest queued write, the only one handed to the writer at a time, so a later one is never written before the outcome of an earlier one is known.
    pub fn head(&self) -> Option<&Job> {
        self.queue.front()
    }

    /// How many writes are queued for the lease.
    pub fn count_for(&self, lease: u64) -> u32 {
        self.queue.iter().filter(|j| j.lease == Some(lease)).count() as u32
    }

    /// Checks `job` against the latest set and queues it. Err when the set would be invalid or the queue is full.
    pub fn admit(&mut self, specs: &[Spec], job: Job) -> Result<(), (InputError, Job)> {
        if self.queue.len() >= QUEUE {
            return Err((InputError::Busy, job));
        }
        if let Err(e) = validate(specs, &self.latest(), job.idx, &job.chord) {
            return Err((e, job));
        }
        self.queue.push_back(job);
        Ok(())
    }

    /// Records that the write for `ticket` finished. A success commits it; a failure drops it and re-checks what queued behind, which may now fail too. Returns every job that settled.
    pub fn finish(&mut self, specs: &[Spec], ticket: u64, outcome: Result<(), String>) -> Vec<Settled> {
        let Some(at) = self.queue.iter().position(|j| j.ticket == ticket) else { return Vec::new() };
        let Some(job) = self.queue.remove(at) else { return Vec::new() };
        match outcome {
            Ok(()) => {
                self.committed[job.idx] = Some(job.chord);
                vec![Settled { job, result: Ok(()) }]
            }
            Err(why) => {
                let mut settled = vec![Settled { job, result: Err(InputError::Persistence(why)) }];
                settled.extend(self.rebase(specs));
                settled
            }
        }
    }

    /// Re-checks the queue in order against the committed set; a job that no longer fits is dropped and reported.
    fn rebase(&mut self, specs: &[Spec]) -> Vec<Settled> {
        let mut set = self.committed.clone();
        let mut kept = VecDeque::new();
        let mut dropped = Vec::new();
        for job in std::mem::take(&mut self.queue) {
            match validate(specs, &set, job.idx, &job.chord) {
                Ok(()) => {
                    set[job.idx] = Some(job.chord);
                    kept.push_back(job);
                }
                Err(e) => dropped.push(Settled { job, result: Err(e) }),
            }
        }
        self.queue = kept;
        dropped
    }
}

/// The writer thread: created on the first save, ends when the input is dropped.
pub struct Writer {
    tx: Mutex<Option<SyncSender<Write>>>,
}

impl Writer {
    /// A writer that has not started.
    pub fn new() -> Writer {
        Writer { tx: Mutex::new(None) }
    }

    /// Hands a write to the thread, starting it first when needed. `done` is called on the writer thread with the ticket and the result. Err(Busy) when the queue is full, Err(Unavailable) when the thread cannot start.
    pub fn submit(&self, store: &Arc<dyn Store>, done: Done, job: (u64, &'static str, Chord)) -> Result<(), InputError> {
        let mut slot = self.tx.lock().unwrap_or_else(|e| e.into_inner());
        if slot.is_none() {
            let (tx, rx) = mpsc::sync_channel::<Write>(QUEUE);
            let store = store.clone();
            crate::guard::spawn("cue-input-save", 256 * 1024, move || {
                while let Ok((ticket, id, chord)) = rx.recv() {
                    let result = store.save(id, &chord);
                    done(ticket, result);
                }
            })
            .map_err(|e| InputError::Unavailable(format!("Cue could not start saving shortcuts: {e}")))?;
            *slot = Some(tx);
        }
        match slot.as_ref().map(|tx| tx.try_send(job)) {
            Some(Err(TrySendError::Full(_))) => Err(InputError::Busy),
            Some(Err(TrySendError::Disconnected(_))) | None => Err(InputError::Unavailable("Cue stopped saving shortcuts.".into())),
            Some(Ok(())) => Ok(()),
        }
    }
}
