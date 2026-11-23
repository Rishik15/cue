//! Purpose: Keep Cue current. A check finds a newer release, the installer downloads and is verified in the background while Cue keeps working, and only when the user chooses to install does Cue stop, run the installer and come back by itself.
//! Contents: transition / set_percent — change the state and emit `cue-update`; check — look on GitHub and start the download; init — one background check at launch (after the first run, setting `check_updates`, no timer); install — re-verify, run the installer, quit; ready_version — the version waiting to be installed (the settings window asks); check_update / update_status / install_update — commands for the About page. Siblings: phase (state and pure transitions), manifest (GitHub and trust), download (background fetch), announce (after-update notice).

mod announce;
mod download;
mod manifest;
mod phase;

use crate::lock::lock;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use tauri::{AppHandle, Emitter};

use crate::{guard, settings};
use manifest::{fetch_manifest, installer_url, newer, verify_file, Manifest};

pub use announce::announce_if_updated;
pub use phase::Snapshot;
use phase::{apply, settle, snapshot_of, Keep, Phase};

static STATE: Mutex<Phase> = Mutex::new(Phase::None);
/// One check at a time: the launch check and the About page's must not both tidy the folder while a download writes into it.
static CHECKING: AtomicBool = AtomicBool::new(false);

struct Checking;

impl Drop for Checking {
    fn drop(&mut self) {
        CHECKING.store(false, Ordering::Release);
    }
}

fn snapshot() -> Snapshot {
    snapshot_of(&lock(&STATE))
}

/// Replaces the phase when `change` returns one, and tells the windows (the edge: `apply` holds the lock, not this emit).
fn transition(app: &AppHandle, change: impl FnOnce(&Phase) -> Option<Phase>) -> bool {
    let Some(snap) = apply(&STATE, change) else { return false };
    let _ = app.emit("cue-update", snap);
    true
}

/// Progress while downloading; emits only when the whole percent changed.
fn set_percent(app: &AppHandle, percent: u8) {
    let snap = {
        let mut state = lock(&STATE);
        let Phase::Downloading { percent: current, .. } = &mut *state else { return };
        if *current == percent {
            return;
        }
        *current = percent;
        snapshot_of(&state)
    };
    let _ = app.emit("cue-update", snap);
}

/// Asks GitHub; a newer release starts downloading (or is ready at once when an earlier run already saved it).
fn check(app: &AppHandle) -> Result<Snapshot, String> {
    if CHECKING.swap(true, Ordering::AcqRel) {
        return Ok(snapshot());
    }
    let _only = Checking;
    if matches!(*lock(&STATE), Phase::Downloading { .. } | Phase::Installing) {
        return Ok(snapshot());
    }
    let local = app.package_info().version.to_string();
    let found = fetch_manifest()?.filter(|m| newer(&m.version, &local));
    if let Some(m) = &found {
        if installer_url(&m.version).as_deref() != Some(m.url.as_str()) {
            return Err("The update does not come from Cue's GitHub page, so it was not downloaded.".into());
        }
    }
    let on_disk = found.as_ref().and_then(|m| download::existing(app, m));
    // shortcut: tidy runs under the state lock, a handful of unlinks, move out if it ever grows.
    let changed = transition(app, |now| {
        let settled = settle(now, found.as_ref(), on_disk);
        download::tidy(app, settled.keep);
        settled.next
    });
    if let (true, Some(manifest)) = (changed, found) {
        if matches!(*lock(&STATE), Phase::Downloading { .. }) {
            download::start(app, manifest);
        }
    }
    Ok(snapshot())
}

/// One check at launch, off the main thread; a failure stays silent (the About page checks again on request). Nothing goes out before the first run is finished.
pub fn init(app: &AppHandle) {
    if !settings::get_bool("onboarded", false) || !settings::get_bool("check_updates", true) {
        download::tidy(app, Keep::All);
        return;
    }
    let app = app.clone();
    let _ = guard::spawn("update-check", 1024 * 1024, move || {
        let _ = check(&app);
    });
}

/// The version of an update that is downloaded and waiting to be installed.
pub fn ready_version() -> Option<String> {
    match &*lock(&STATE) {
        Phase::Ready { manifest, .. } => Some(manifest.version.clone()),
        _ => None,
    }
}

/// Takes the Ready update and marks it Installing in one step, so a second press cannot start another.
fn claim(app: &AppHandle) -> Result<(Manifest, PathBuf), String> {
    let mut taken = None;
    transition(app, |now| {
        if let Phase::Ready { manifest, path } = now {
            taken = Some((manifest.clone(), path.clone()));
            return Some(Phase::Installing);
        }
        None
    });
    taken.ok_or_else(|| "The update is not ready yet.".to_string())
}

/// Back to Ready after the installer could not be started, so the user can try again.
fn restore(app: &AppHandle, manifest: Manifest, path: PathBuf) {
    transition(app, |_| Some(Phase::Ready { manifest, path }));
}

/// Last look before running the file: still newer, still from our address, and the bytes on disk are still the signed ones.
fn prepare(app: &AppHandle, manifest: &Manifest, path: &std::path::Path) -> Result<(), String> {
    if !newer(&manifest.version, &app.package_info().version.to_string()) {
        return Err("The update is not newer than this version, so it was not installed.".into());
    }
    if installer_url(&manifest.version).as_deref() != Some(manifest.url.as_str()) {
        return Err("The update does not come from Cue's GitHub page, so it was not installed.".into());
    }
    verify_file(path, &manifest.signature)
}

const RELEASES: &str = "https://github.com/Rishik15/cue/releases/latest";

/// Whether this copy lives under the user's own folder (where the installer puts it, no administrator needed). Both paths are resolved first, so a short 8.3 name or a link cannot hide it.
fn is_per_user() -> bool {
    let resolve = |p: std::path::PathBuf| std::fs::canonicalize(p).ok();
    let exe = std::env::current_exe().ok().and_then(resolve);
    let local = std::env::var_os("LOCALAPPDATA").map(std::path::PathBuf::from).and_then(resolve);
    matches!((exe, local), (Some(e), Some(l)) if under(&e, &l))
}

/// Component by component, ignoring case: `...\Local2` is not under `...\Local`.
fn under(exe: &std::path::Path, dir: &std::path::Path) -> bool {
    let lower = |p: &std::path::Path| std::path::PathBuf::from(p.to_string_lossy().to_lowercase());
    lower(exe).starts_with(lower(dir))
}

/// Starts the installer silently (no prompt, no window) and quits so it can replace the program; its /R brings Cue back. An all-users copy, which needs administrator rights, is sent to the browser instead.
/// shortcut: Cue quits as soon as the installer process starts and does not wait for its exit code, so an installer that fails after launch leaves no running app and no message; the signed installer stays in the update folder, so the next launch (Start menu, login entry) finds it Ready and the update can be tried again, and the old files are intact unless NSIS failed mid-copy. Upgrade path: keep Cue alive until the installer exits (needs the installer to close Cue itself) and restart from the old exe on a non-zero exit code; justified once a failing-installer run (denied replace access) shows a damaged install, driven from `scripts/verify/upgrade-check.ps1`.
fn install(app: &AppHandle) -> Result<(), String> {
    let (manifest, path) = claim(app)?;
    if let Err(e) = prepare(app, &manifest, &path) {
        let _ = std::fs::remove_file(&path);
        transition(app, |_| Some(Phase::None));
        return Err(e);
    }
    if !is_per_user() {
        // The installer is per user: run on an all-users install it would leave a second copy. Windows' own folder needs the user's hands.
        restore(app, manifest, path);
        let _ = crate::platform::open_url(RELEASES);
        return Err("Cue is installed for all users, so the update opens in your browser. Run it there.".into());
    }
    // Started directly, not through the shell: no SmartScreen or "Open File" prompt on the unsigned installer. /S no window, /R start Cue again afterwards, /UPDATE replace a running install.
    match crate::platform::run_installer(&path, "/S /R /UPDATE") {
        Ok(true) => {
            app.exit(0);
            Ok(())
        }
        // Windows wants administrator rights: an all-users install we read as per-user. Send the user to the download page instead of failing silently.
        Ok(false) => {
            restore(app, manifest, path);
            let _ = crate::platform::open_url(RELEASES);
            Err("This copy of Cue needs administrator rights to update, so the update opens in your browser. Run it there.".into())
        }
        Err(e) => {
            restore(app, manifest, path);
            Err(e)
        }
    }
}

/// Checks now; a newer version starts downloading in the background.
#[tauri::command]
pub async fn check_update(app: AppHandle) -> Result<Snapshot, String> {
    tauri::async_runtime::spawn_blocking(move || check(&app)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn update_status() -> Snapshot {
    snapshot()
}

#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || install(&app)).await.map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests;
