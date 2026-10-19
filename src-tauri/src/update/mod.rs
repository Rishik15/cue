//! Purpose: Keep Cue current. A check finds a newer release, the installer downloads and is verified in the background while Cue keeps working, and only when the user chooses to install does Cue stop, run the installer and come back by itself.
//! Contents: Phase / Snapshot — the update state and what the windows see; transition / set_percent — change the state and emit `cue-update`; check — look on GitHub and start the download; init — one background check at launch (after the first run, setting `check_updates`, no timer); install — re-verify, run the installer, quit; ready_version — the version waiting to be installed (the tray menu and the settings window ask); check_update / update_status / install_update — commands for the About page and the update window. Siblings: manifest (GitHub and trust), download (background fetch), announce (after-update notice).

mod announce;
mod download;
mod manifest;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::{guard, settings};
use manifest::{fetch_manifest, installer_url, newer, verify_file, Manifest};

pub use announce::announce_if_updated;

enum Phase {
    None,
    Downloading { manifest: Manifest, percent: u8 },
    Ready { manifest: Manifest, path: PathBuf },
    Installing,
}

/// What the windows see; also the payload of the `cue-update` event.
#[derive(Clone, Serialize)]
pub struct Snapshot {
    phase: &'static str,
    version: Option<String>,
    percent: u8,
}

static STATE: Mutex<Phase> = Mutex::new(Phase::None);
/// One check at a time: the launch check and the About page's must not both tidy the folder while a download writes into it.
static CHECKING: AtomicBool = AtomicBool::new(false);

struct Checking;

impl Drop for Checking {
    fn drop(&mut self) {
        CHECKING.store(false, Ordering::Release);
    }
}

fn snapshot_of(phase: &Phase) -> Snapshot {
    let (name, version, percent) = match phase {
        Phase::None => ("none", None, 0),
        Phase::Downloading { manifest, percent } => ("downloading", Some(manifest.version.clone()), *percent),
        Phase::Ready { manifest, .. } => ("ready", Some(manifest.version.clone()), 100),
        Phase::Installing => ("installing", None, 100),
    };
    Snapshot { phase: name, version, percent }
}

fn snapshot() -> Snapshot {
    snapshot_of(&STATE.lock().unwrap())
}

/// Replaces the phase when `change` returns one, and tells the windows. The lock is not held while emitting.
fn transition(app: &AppHandle, change: impl FnOnce(&Phase) -> Option<Phase>) -> bool {
    let snap = {
        let mut state = STATE.lock().unwrap();
        let Some(next) = change(&state) else { return false };
        *state = next;
        snapshot_of(&state)
    };
    let _ = app.emit("cue-update", snap);
    true
}

/// Progress while downloading; emits only when the whole percent changed.
fn set_percent(app: &AppHandle, percent: u8) {
    let snap = {
        let mut state = STATE.lock().unwrap();
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
    if matches!(*STATE.lock().unwrap(), Phase::Downloading { .. } | Phase::Installing) {
        return Ok(snapshot());
    }
    let local = app.package_info().version.to_string();
    let Some(manifest) = fetch_manifest()?.filter(|m| newer(&m.version, &local)) else { return Ok(snapshot()) };
    if installer_url(&manifest.version).as_deref() != Some(manifest.url.as_str()) {
        return Err("The update does not come from Cue's GitHub page, so it was not downloaded.".into());
    }
    download::tidy(app, Some(&manifest.version));
    let phase = match download::existing(app, &manifest) {
        Some(path) => Phase::Ready { manifest: manifest.clone(), path },
        None => Phase::Downloading { manifest: manifest.clone(), percent: 0 },
    };
    let downloading = matches!(phase, Phase::Downloading { .. });
    let changed = transition(app, |now| (!matches!(now, Phase::Downloading { .. } | Phase::Installing)).then_some(phase));
    if changed && downloading {
        download::start(app, manifest);
    }
    Ok(snapshot())
}

/// One check at launch, off the main thread; a failure stays silent (the About page checks again on request). Nothing goes out before the first run is finished.
pub fn init(app: &AppHandle) {
    if !settings::get_bool(app, "onboarded", false) || !settings::get_bool(app, "check_updates", true) {
        download::tidy(app, None);
        return;
    }
    let app = app.clone();
    let _ = guard::spawn("update-check", 1024 * 1024, move || {
        let _ = check(&app);
    });
}

/// The version of an update that is downloaded and waiting to be installed.
pub fn ready_version() -> Option<String> {
    match &*STATE.lock().unwrap() {
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

/// Starts the installer and quits so it can replace the program; its /R brings Cue back.
fn install(app: &AppHandle) -> Result<(), String> {
    let (manifest, path) = claim(app)?;
    if let Err(e) = prepare(app, &manifest, &path) {
        let _ = std::fs::remove_file(&path);
        transition(app, |_| Some(Phase::None));
        return Err(e);
    }
    // /P quiet with progress, /R start Cue again afterwards, /UPDATE tell the installer this replaces a running install.
    match crate::platform::start_program(&path, "/P /R /UPDATE") {
        Ok(true) => {
            app.exit(0);
            Ok(())
        }
        Ok(false) => {
            restore(app, manifest, path);
            Err("Windows asked for permission and it was declined, so Cue was not updated.".into())
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
