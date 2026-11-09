//! File: model_download/worker.rs
//! Purpose: The work of one download: fetch each file in order (taking the lock of a file other models share), then run what follows.
//! Contents: run — the ordered work, ending with the reason a step stopped; rebook / check / stopped — helpers.

use std::path::PathBuf;

use super::ctl::{Ctl, Phase};
use super::job::Env;
use super::ledger::path_key;
use super::transfer::{self, failed, Stop};
use super::{After, Wanted};

/// Whether the file was unnecessary after waiting for its lock: whoever held it did the work already.
fn already_done(w: &Wanted) -> bool {
    w.shared.as_ref().and_then(|s| s.skip_if.as_ref()).is_some_and(|p: &PathBuf| p.exists())
}

/// Fetches every file in `wanted` (shared ones under their lock, with their follow-up step while the lock is held), then runs `after`; ends early with the reason a step stopped.
pub(super) fn run(env: &Env, wanted: &[Wanted], after: After, ctl: &Ctl, on_progress: &dyn Fn(u64, u64)) -> Result<(), Stop> {
    let total: u64 = wanted.iter().map(|w| w.file.size).sum();
    let mut base = 0u64;
    for w in wanted {
        let _held = match &w.shared {
            Some(_) => Some(env.manager.lock(path_key(&w.dir, w.file.name), ctl).map_err(|()| Stop::Cancelled)?),
            None => None,
        };
        if w.shared.is_some() {
            rebook(env, w, ctl)?;
        }
        if !already_done(w) {
            ctl.set_phase(Phase::Fetching);
            env.disk.create_dir_all(&w.dir).map_err(failed("Could not create the models folder"))?;
            let mut report = |file_bytes: u64| on_progress(base + file_bytes, total);
            transfer::fetch_retrying(env.net.as_ref(), &w.url, w.file, &w.dir, ctl, &mut report)?;
            if let Some(then) = w.shared.as_ref().and_then(|s| s.then.as_ref()) {
                check(ctl)?;
                ctl.set_phase(Phase::Installing);
                then(ctl).map_err(|e| stopped(ctl, e))?;
            }
        }
        base += w.file.size;
    }
    check(ctl)?;
    ctl.set_phase(Phase::Installing);
    after(ctl).map_err(|e| stopped(ctl, e))
}

/// Plans the file again from the disk as it is now and updates the disk books, because whoever held its lock before may have finished, discarded or cancelled it.
fn rebook(env: &Env, w: &Wanted, ctl: &Ctl) -> Result<(), Stop> {
    ctl.retarget(&w.claims(), &|| env.disk.free_room(&w.dir)).map_err(Stop::Failed)
}

/// Ends the download when a cancel arrived before a step that cannot be undone.
fn check(ctl: &Ctl) -> Result<(), Stop> {
    if ctl.is_cancelled() {
        return Err(Stop::Cancelled);
    }
    Ok(())
}

/// Why a step failed: a cancel that killed its work is a cancel, not a failure.
fn stopped(ctl: &Ctl, error: String) -> Stop {
    if ctl.is_cancelled() {
        Stop::Cancelled
    } else {
        Stop::Failed(error)
    }
}
