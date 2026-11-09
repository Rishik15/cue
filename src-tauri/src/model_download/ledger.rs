//! File: model_download/ledger.rs
//! Purpose: The disk-room books of downloads, keyed by path: how many bytes each file still has to write and which downloads rely on it. No I/O and no clock: `step` applies one `Op` to a `Ledger`, so every interleaving can be tested by feeding ops in order.
//! Contents: path_key — the key of a file; Claim — what a download needs for one path; Op — the changes; Ledger — the books; step — apply one op; Short — a refusal with the shortfall.
//! A path shared by two downloads (an archive two models unpack) is reserved once. Its reservation survives the cancel of the download that started it, as long as another download still relies on it.

use std::collections::{BTreeMap, BTreeSet};

/// The key of a file in the disk books and the file locks: its path, lower case because Windows paths ignore case.
pub fn path_key(dir: &std::path::Path, name: &str) -> String {
    dir.join(name).to_string_lossy().to_lowercase()
}

/// The download a claim belongs to: model id and generation.
pub type JobKey = (&'static str, u64);

/// Bytes one download still has to write for one path (a file, or `<file>#unpack` for the room unpacking it takes).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Claim {
    pub key: String,
    pub bytes: u64,
}

/// A change to the books. `free` is the room on the drive, read under the same lock as the op.
pub enum Op<'a> {
    /// A download starts: raises each claimed path to at least the claim, counting a path someone holds already only once.
    Admit { job: JobKey, claims: &'a [Claim], free: u64 },
    /// A download re-plans from the disk as it is now (at start, or when it was handed a shared path): sets each claimed path to exactly the claim. Never refused when it takes off at least as much as it adds.
    Retarget { job: JobKey, claims: &'a [Claim], free: u64 },
    /// `bytes` were written to `key`.
    Wrote { key: &'a str, bytes: u64 },
    /// `bytes` already on disk for `key` are about to be thrown away and written again. Recorded before the delete, so room freed by it is never seen without the debt.
    Lose { key: &'a str, bytes: u64 },
    /// A download ended; paths nobody relies on any more are dropped.
    Release { job: JobKey },
}

/// A refusal: the claims need this many bytes more than the drive has.
#[derive(Debug, PartialEq, Eq)]
pub struct Short(pub u64);

/// One path in the books: the bytes still to write and the downloads relying on it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Entry {
    reserved: u64,
    holders: BTreeSet<JobKey>,
}

/// What every running download still has to write, per path.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Ledger {
    entries: BTreeMap<String, Entry>,
}

impl Ledger {
    /// Empty books.
    pub const fn new() -> Ledger {
        Ledger { entries: BTreeMap::new() }
    }

    /// Bytes all paths still have to write.
    pub fn debt(&self) -> u64 {
        self.entries.values().map(|e| e.reserved).sum()
    }

    /// Bytes reserved for `key` (0 when nobody relies on it).
    pub fn reserved(&self, key: &str) -> u64 {
        self.entries.get(key).map_or(0, |e| e.reserved)
    }

    /// Whether `job` relies on `key` (tests read the books through it).
    #[cfg(test)]
    pub fn holds(&self, job: JobKey, key: &str) -> bool {
        self.entries.get(key).is_some_and(|e| e.holders.contains(&job))
    }

    /// Bytes the claims would add to the debt.
    fn added(&self, claims: &[Claim]) -> u64 {
        claims.iter().map(|c| c.bytes.saturating_sub(self.reserved(&c.key))).sum()
    }

    /// Bytes the claims would take off the debt.
    fn removed(&self, claims: &[Claim]) -> u64 {
        claims.iter().map(|c| self.reserved(&c.key).saturating_sub(c.bytes)).sum()
    }

    /// Refuses when the debt plus `added` does not fit in `free`.
    fn fits(&self, added: u64, free: u64) -> Result<(), Short> {
        match self.debt().saturating_add(added).checked_sub(free) {
            Some(over) if over > 0 => Err(Short(over)),
            _ => Ok(()),
        }
    }

    /// Records the claims for `job`; `exact` sets a path to its claim, otherwise a path only grows.
    fn apply(&mut self, job: JobKey, claims: &[Claim], exact: bool) {
        for claim in claims {
            let entry = self.entries.entry(claim.key.clone()).or_default();
            entry.reserved = if exact { claim.bytes } else { entry.reserved.max(claim.bytes) };
            entry.holders.insert(job);
        }
    }
}

/// Applies `op` to `ledger`. An `Err` leaves the books as they were.
pub fn step(ledger: &mut Ledger, op: Op<'_>) -> Result<(), Short> {
    match op {
        Op::Admit { job, claims, free } => {
            ledger.fits(ledger.added(claims), free)?;
            ledger.apply(job, claims, false);
        }
        Op::Retarget { job, claims, free } => {
            let (grown, shrunk) = (ledger.added(claims), ledger.removed(claims));
            if grown > shrunk {
                ledger.fits(grown - shrunk, free)?;
            }
            ledger.apply(job, claims, true);
        }
        Op::Wrote { key, bytes } => {
            if let Some(entry) = ledger.entries.get_mut(key) {
                entry.reserved = entry.reserved.saturating_sub(bytes);
            }
        }
        Op::Lose { key, bytes } => {
            if let Some(entry) = ledger.entries.get_mut(key) {
                entry.reserved = entry.reserved.saturating_add(bytes);
            }
        }
        Op::Release { job } => {
            ledger.entries.values_mut().for_each(|e| {
                e.holders.remove(&job);
            });
            ledger.entries.retain(|_, e| !e.holders.is_empty());
        }
    }
    Ok(())
}
