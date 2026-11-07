//! Purpose: The thread of one download: fetch each file in order (taking the lock of a file other models share), run what follows, and report how it ended.
//! Contents: run — the work of one download; Outcome — how it ended; steps — the ordered work behind `run`.

use std::fs;
use std::path::PathBuf;

use tauri::AppHandle;

use super::state::{Ctl, Manager};
use super::transfer::{self, failed, Stop};
use super::{After, Wanted};

/// How a download ended.
pub(super) enum Outcome {
    Succeeded,
    Cancelled,
    Failed(String),
}

/// The key of a shared file: its path, lower case because Windows paths ignore case.
fn lock_key(w: &Wanted) -> String {
    w.dir.join(w.file.name).to_string_lossy().to_lowercase()
}

/// Whether the file was unnecessary after waiting for its lock: whoever held it did the work already.
fn already_done(w: &Wanted) -> bool {
    w.shared.as_ref().and_then(|s| s.skip_if.as_ref()).is_some_and(|p: &PathBuf| p.exists())
}

/// Fetches every file in `wanted` (shared ones under their lock, with their follow-up step while the lock is held), then runs `after`.
pub(super) fn run(
    manager: &'static Manager,
    app: &AppHandle,
    wanted: &[Wanted],
    after: After,
    ctl: &Ctl,
    on_progress: &dyn Fn(u64, u64),
) -> Outcome {
    match steps(manager, app, wanted, after, ctl, on_progress) {
        Ok(()) => Outcome::Succeeded,
        Err(Stop::Cancelled) => Outcome::Cancelled,
        Err(Stop::Failed(e) | Stop::Offline(e)) => Outcome::Failed(e),
    }
}

/// The steps of `run`, ending early with the reason a step stopped.
fn steps(
    manager: &'static Manager,
    app: &AppHandle,
    wanted: &[Wanted],
    after: After,
    ctl: &Ctl,
    on_progress: &dyn Fn(u64, u64),
) -> Result<(), Stop> {
    let total: u64 = wanted.iter().map(|w| w.file.size).sum();
    let mut base = 0u64;
    for w in wanted {
        let _held = match &w.shared {
            Some(_) => Some(manager.lock(lock_key(w), ctl).map_err(|()| Stop::Cancelled)?),
            None => None,
        };
        if !already_done(w) {
            fs::create_dir_all(&w.dir).map_err(failed("Could not create the models folder"))?;
            let mut report = |file_bytes: u64| on_progress(base + file_bytes, total);
            transfer::fetch_retrying(&w.url, w.file, &w.dir, ctl, &mut report)?;
            if let Some(then) = w.shared.as_ref().and_then(|s| s.then) {
                check(ctl)?;
                then(app).map_err(Stop::Failed)?;
            }
        }
        base += w.file.size;
    }
    check(ctl)?;
    after(app).map_err(Stop::Failed)
}

/// Ends the download when a cancel arrived before a step that cannot be undone.
fn check(ctl: &Ctl) -> Result<(), Stop> {
    if ctl.is_cancelled() {
        return Err(Stop::Cancelled);
    }
    Ok(())
}
