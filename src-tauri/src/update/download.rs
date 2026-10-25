//! Purpose: Fetch and verify an installer in the background, into `<app data>/updates/`, and keep that folder tidy.
//! Contents: file_name / is_legacy — pure naming rules; tidy — delete everything but the wanted installer; existing — a finished, verified download from an earlier run; start — background thread that downloads, verifies and renames; fail — drop back to no update and tell the user.

use std::io::{Read, Write};
use std::path::PathBuf;

use tauri::{AppHandle, Manager};

use super::manifest::{get, verify_file, Manifest};
use super::{transition, Phase};
use crate::guard;

const MAX_INSTALLER: u64 = 100 * 1024 * 1024;

/// Deterministic, so a finished download survives a restart.
fn file_name(version: &str) -> String {
    format!("Cue_{version}_x64-setup.exe")
}

/// Installers that older versions of Cue saved straight into the data folder.
fn is_legacy(name: &str) -> bool {
    name.starts_with("cue-update-setup-") && name.ends_with(".exe")
}

fn updates_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?.join("updates");
    std::fs::create_dir_all(&dir).map_err(|e| format!("Could not create the update folder: {e}"))?;
    Ok(dir)
}

/// Best effort (a running installer stays locked). Keeps only the finished installer of `keep`.
pub(super) fn tidy(app: &AppHandle, keep: Option<&str>) {
    let Ok(data) = app.path().app_data_dir() else { return };
    let keep = keep.map(file_name);
    for (folder, legacy_only) in [(data.clone(), true), (data.join("updates"), false)] {
        let Ok(entries) = std::fs::read_dir(folder) else { continue };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let doomed = if legacy_only { is_legacy(&name) } else { keep.as_deref() != Some(name.as_str()) };
            if doomed {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
}

/// The installer from an earlier run, when it is still whole and signed.
pub(super) fn existing(app: &AppHandle, manifest: &Manifest) -> Option<PathBuf> {
    let path = updates_dir(app).ok()?.join(file_name(&manifest.version));
    if !path.exists() {
        return None;
    }
    if verify_file(&path, &manifest.signature).is_ok() {
        return Some(path);
    }
    let _ = std::fs::remove_file(&path);
    None
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
                let prefs = crate::prefs::Prefs::load(&worker);
                crate::platform::overlay::notice("Cue update ready, open Cue to install", prefs.overlay_top, prefs.theme());
            }
            Err(e) => fail(&worker, &e),
        }
    });
    if let Err(e) = started {
        fail(app, &e.to_string());
    }
}

fn fail(app: &AppHandle, detail: &str) {
    crate::log::note(&format!("update download failed: {detail}"));
    transition(app, |_| Some(Phase::None));
    crate::status::report_in_app(app, "Could not download the update. Check for updates to try again.");
}

/// Downloads to `.part`, verifies it on disk, then renames it to its final name.
fn fetch(app: &AppHandle, manifest: &Manifest) -> Result<PathBuf, String> {
    let name = file_name(&manifest.version);
    let dir = updates_dir(app)?;
    let (part, done) = (dir.join(format!("{name}.part")), dir.join(&name));
    let _ = std::fs::remove_file(&part);
    let result = write_part(app, manifest, &part)
        .and_then(|()| verify_file(&part, &manifest.signature))
        .and_then(|()| std::fs::rename(&part, &done).map_err(|e| format!("Could not save the update: {e}")));
    if result.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    result.map(|()| done)
}

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
    use super::{file_name, is_legacy};

    #[test]
    fn names_are_fixed_and_old_ones_recognised() {
        assert_eq!(file_name("0.2.0"), "Cue_0.2.0_x64-setup.exe");
        assert!(is_legacy("cue-update-setup-00ff.exe"));
        assert!(!is_legacy("settings.json") && !is_legacy("Cue_0.2.0_x64-setup.exe"));
    }
}
