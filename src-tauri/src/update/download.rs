//! File: update/download.rs
//! Purpose: Fetch and verify an installer in the background, into `<app data>/updates/`, and keep that folder tidy.
//! Contents: file_name / doomed — pure naming and cleanup rules; tidy — delete what the phase does not reference; existing — a finished, verified download from an earlier run; start — background thread that downloads, verifies and renames; fail — drop back to no update and tell the user.

use std::io::{Read, Write};
use std::path::PathBuf;

use tauri::{AppHandle, Manager};

use super::manifest::{get, verify_file, Manifest};
use super::phase::Keep;
use super::{transition, Phase};
use crate::guard;

const MAX_INSTALLER: u64 = 100 * 1024 * 1024;

/// Deterministic, so a finished download survives a restart.
fn file_name(version: &str) -> String {
    format!("Cue_{version}_x64-setup.exe")
}

/// Whether cleanup deletes `name` from the update folder under `keep`.
fn doomed(name: &str, keep: &Keep) -> bool {
    match keep {
        Keep::All => true,
        Keep::File(version) => name != file_name(version),
        Keep::Untouched => false,
    }
}

/// The update folder, created when missing. Input: the app handle. Returns its path, or a user-readable error.
fn updates_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?.join("updates");
    std::fs::create_dir_all(&dir).map_err(|e| format!("Could not create the update folder: {e}"))?;
    Ok(dir)
}

/// Best effort (a running installer stays locked). Deletes what `keep` does not protect from the update folder.
pub(super) fn tidy(app: &AppHandle, keep: Keep) {
    if keep == Keep::Untouched {
        return;
    }
    let Ok(data) = app.path().app_data_dir() else { return };
    let Ok(entries) = std::fs::read_dir(data.join("updates")) else { return };
    for entry in entries.flatten() {
        if doomed(&entry.file_name().to_string_lossy(), &keep) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// The installer from an earlier run, when it is still whole and signed. Read-only: a file that fails stays until the download replaces it, so a probe never deletes what a phase may reference.
pub(super) fn existing(app: &AppHandle, manifest: &Manifest) -> Option<PathBuf> {
    probe(&updates_dir(app).ok()?.join(file_name(&manifest.version)), manifest)
}

/// `path` when it is a file that passes the manifest's signature check; never touches it.
fn probe(path: &std::path::Path, manifest: &Manifest) -> Option<PathBuf> {
    (path.exists() && verify_file(path, &manifest.signature).is_ok()).then(|| path.to_path_buf())
}

/// Runs the download on its own thread (1 MB: the TLS handshake needs more than the usual small stack).
pub(super) fn start(app: &AppHandle, manifest: Manifest) {
    let worker = app.clone();
    let started = guard::spawn("update-download", 1024 * 1024, move || {
        crate::platform::background_thread();
        // A crash in the download must not leave the update stuck on "downloading" with nothing running.
        match guard::contain(|| fetch(&worker, &manifest)).unwrap_or_else(|| Err("the download crashed".into())) {
            Ok(path) => {
                transition(&worker, |_| Some(Phase::Ready { manifest, path }));
                let prefs = crate::prefs::Prefs::load();
                crate::platform::overlay::notice("Cue update ready, open Cue to install", prefs.overlay_top, prefs.theme());
            }
            Err(e) => fail(&worker, &e),
        }
    });
    if let Err(e) = started {
        fail(app, &e.to_string());
    }
}

/// Drops back to no update, logs the cause and tells the user. Inputs: the app handle and the log detail. Returns nothing.
fn fail(app: &AppHandle, detail: &str) {
    crate::log::note(&format!("update download failed: {detail}"));
    transition(app, |_| Some(Phase::None));
    crate::status::report_in_app(app, "Could not download the update. Check for updates to try again.");
}

/// Downloads to `.part`, verifies it on disk, then syncs and renames it to its final name (`fsutil::commit`).
fn fetch(app: &AppHandle, manifest: &Manifest) -> Result<PathBuf, String> {
    let name = file_name(&manifest.version);
    let dir = updates_dir(app)?;
    let (part, done) = (dir.join(format!("{name}.part")), dir.join(&name));
    let _ = std::fs::remove_file(&part);
    let result = write_part(app, manifest, &part)
        .and_then(|()| verify_file(&part, &manifest.signature))
        .and_then(|()| crate::fsutil::commit(&part, &done).map_err(|e| format!("Could not save the update: {e}")));
    if result.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    result.map(|()| done)
}

/// Streams the installer into `part` (at most `MAX_INSTALLER` bytes), reporting whole-percent progress. Inputs: the app handle, the manifest and the new file's path. Returns an error when the request, the write or the length check fails.
fn write_part(app: &AppHandle, manifest: &Manifest, part: &std::path::Path) -> Result<(), String> {
    let response = get(&manifest.url).map_err(|code| format!("status {code:?}"))?;
    let total =
        response.headers().get("Content-Length").and_then(|v| v.to_str().ok()).and_then(|v| v.parse::<u64>().ok()).filter(|&t| t > 0);
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(part).map_err(|e| e.to_string())?;
    let mut reader = response.into_body().into_reader().take(MAX_INSTALLER);
    let (mut chunk, mut received, mut last) = (vec![0u8; 64 * 1024], 0u64, 0u8);
    loop {
        let n = reader.read(&mut chunk).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        file.write_all(&chunk[..n]).map_err(|e| e.to_string())?;
        received += n as u64;
        let percent = total.map_or(0, |t| (received * 100 / t).min(100) as u8);
        if percent != last {
            last = percent;
            super::set_percent(app, percent);
        }
    }
    file.flush().map_err(|e| e.to_string())?;
    match total {
        Some(t) if t != received => Err("the download was cut short".into()),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::{doomed, file_name, probe};
    use crate::update::manifest::Manifest;
    use crate::update::phase::Keep;

    #[test]
    fn names_are_fixed_and_cleanup_keeps_only_the_named_installer() {
        assert_eq!(file_name("0.2.0"), "Cue_0.2.0_x64-setup.exe");
        let keep = Keep::File("0.2.0".into());
        assert!(!doomed("Cue_0.2.0_x64-setup.exe", &keep));
        assert!(doomed("Cue_0.1.9_x64-setup.exe", &keep) && doomed("Cue_0.2.0_x64-setup.exe.part", &keep));
        assert!(doomed("Cue_0.2.0_x64-setup.exe", &Keep::All));
        assert!(!doomed("anything", &Keep::Untouched));
    }

    /// A same-version installer that fails its signature check is not deleted by the probe, so a claimed or Ready file survives a check.
    #[test]
    fn a_failed_probe_leaves_the_file() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join(file_name("0.2.0"));
        std::fs::write(&path, b"installer").expect("write");
        let manifest = Manifest { version: "0.2.0".into(), url: String::new(), signature: "AAAA".into() };
        assert_eq!(probe(&path, &manifest), None);
        assert!(path.exists());
    }
}
