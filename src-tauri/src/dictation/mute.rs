//! File: mute.rs
//! Purpose: Mute the speakers while dictating (setting "Mute Audio While Recording") and always give them back.
//! Contents:
//! - init — remember the app handle and undo a mute a crash left behind
//! - start / stop — mute at the start of a recording, restore at its end
//! - release — restore at once and refuse further muting, for Quit
//!
//! Cue restores only what it muted: speakers already muted stay muted. The muted device is written to a file while held, so the
//! next launch can restore it if Cue died in between. A file, not a setting: saving a setting starts the store plugin's async runtime (8 idle threads).

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use tauri::{AppHandle, Manager};

use crate::{platform, settings, status};

const MARKER: &str = "muted.endpoint";
/// How long Quit waits for a mute worker that holds `HELD` before giving up.
const QUIT_WAIT: std::time::Duration = std::time::Duration::from_secs(2);

static APP: OnceLock<AppHandle> = OnceLock::new();
/// The device Cue is holding muted. The lock also orders `start`, `stop` and `release`, which run on their own threads.
static HELD: Mutex<Option<String>> = Mutex::new(None);
/// Set by `release`: the app is quitting, so no later `start` may mute again.
static QUITTING: AtomicBool = AtomicBool::new(false);

/// The file that remembers the muted device across a crash. Input: the app handle. Return: its path, None without a data folder.
fn marker(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_data_dir().ok().map(|dir| dir.join(MARKER))
}

/// Remembers the app handle and restores a mute a killed run left behind. Input: the app handle.
pub fn init(app: &AppHandle) {
    let _ = APP.set(app.clone());
    let Some(id) = marker(app).and_then(|m| std::fs::read_to_string(m).ok()).filter(|id| !id.is_empty()) else { return };
    spawn("unmute", move |app| {
        if let Err(problem) = restore(app, &id) {
            status::report(app, &problem);
        }
    });
}

/// Runs `job` on a named worker thread with the app handle. Input: the thread name and the job.
fn spawn(name: &str, job: impl FnOnce(&AppHandle) + Send + 'static) {
    let Some(app) = APP.get().cloned() else { return };
    if let Err(e) = crate::guard::spawn(name, 256 * 1024, move || job(&app)) {
        if let Some(app) = APP.get() {
            status::report(app, &format!("Could not start the speaker volume helper: {e}"));
        }
    }
}

/// Puts the device back and removes the marker. Input: the app handle and the muted device id. Return: Ok when the speakers are back (the marker is gone); Err with a sentence for the user otherwise, leaving the marker so the next launch or stop retries. Never reports itself: callers report after they release `HELD`, because a report waits for the main thread, which at Quit waits for that lock.
fn restore(app: &AppHandle, id: &str) -> Result<(), String> {
    platform::unmute_output(id).map_err(|e| format!("Could not restore your speaker volume: {e}"))?;
    if let Some(m) = marker(app) {
        let _ = std::fs::remove_file(m);
    }
    Ok(())
}

/// Writes the marker for `id`. Input: the app handle and the device id. Return: a sentence for the user when it could not be written (the mute stays tracked in memory); the caller reports it after releasing `HELD`.
fn remember(app: &AppHandle, id: &str) -> Option<String> {
    let m = marker(app)?;
    std::fs::create_dir_all(m.parent().unwrap_or(&m))
        .and_then(|()| std::fs::write(&m, id))
        .err()
        .map(|e| format!("Cue could not note that it muted your speakers, so a crash would leave them muted: {e}"))
}

/// Recording began. A thread of its own, never the hotkey thread: talking to the audio service takes a few milliseconds. Problems are reported after `HELD` is released.
pub fn start() {
    spawn("mute", |app| {
        if !settings::get_bool(app, "mute_while_recording", true) {
            return;
        }
        let problem = {
            let mut held = HELD.lock().unwrap_or_else(|e| e.into_inner());
            if held.is_some() || !super::is_recording() || QUITTING.load(Ordering::Acquire) {
                return;
            }
            match platform::mute_output() {
                Ok(Some(id)) => {
                    let problem = remember(app, &id);
                    *held = Some(id);
                    problem
                }
                Ok(None) => None,
                Err(e) => Some(format!("Could not mute the speakers: {e}")),
            }
        };
        if let Some(problem) = problem {
            status::report(app, &problem);
        }
    });
}

/// Recording ended (released, cancelled or failed). A restore problem is reported after `HELD` is released.
pub fn stop() {
    spawn("unmute", |app| {
        if let Some(problem) = put_back(app, None) {
            status::report(app, &problem);
        }
    });
}

/// Restores the held device under the lock, keeping it held when that fails. Input: the app handle and how long to wait for the lock (None waits for it). Return: the problem to report, or None when nothing was held, the speakers are back, or the lock could not be had in time (then the marker file stays for the next launch to restore).
fn put_back(app: &AppHandle, wait: Option<std::time::Duration>) -> Option<String> {
    let mut held = match wait {
        None => HELD.lock().unwrap_or_else(|e| e.into_inner()),
        Some(limit) => lock_within(limit)?,
    };
    let id = held.take()?;
    match restore(app, &id) {
        Ok(()) => None,
        Err(problem) => {
            *held = Some(id);
            Some(problem)
        }
    }
}

/// Quit: gives the speakers back before the process ends and stops any later mute. Runs on the calling thread and waits for the audio service, so call it once, at exit; it waits at most `QUIT_WAIT` for a worker that holds the lock. A problem is reported after the lock is released.
pub fn release() {
    QUITTING.store(true, Ordering::Release);
    let Some(app) = APP.get() else { return };
    if let Some(problem) = put_back(app, Some(QUIT_WAIT)) {
        status::report(app, &problem);
    }
}

/// Takes `HELD` within `limit`. Return: the guard, or None (logged) when a worker stuck in the audio service still holds it, so Quit never hangs on it.
fn lock_within(limit: std::time::Duration) -> Option<std::sync::MutexGuard<'static, Option<String>>> {
    let deadline = std::time::Instant::now() + limit;
    loop {
        match HELD.try_lock() {
            Ok(guard) => return Some(guard),
            Err(std::sync::TryLockError::Poisoned(e)) => return Some(e.into_inner()),
            Err(std::sync::TryLockError::WouldBlock) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(10))
            }
            Err(std::sync::TryLockError::WouldBlock) => {
                crate::log::note(
                    "mute: gave up waiting for the audio service at quit; the marker file restores the speakers at the next launch",
                );
                return None;
            }
        }
    }
}
