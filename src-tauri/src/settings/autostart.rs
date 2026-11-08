//! File: settings/autostart.rs
//! Purpose: Launch at Login: the per-user Run entry, read and changed through the autostart plugin, with every failure shown to the user.
//! Contents: get_autostart / set_autostart — the two commands the General page calls; enable_autostart_once — Launch at Login on at first run.

use tauri::{AppHandle, Manager};
use tauri_plugin_autostart::ManagerExt;

/// Whether Launch at Login is on. Input: the app handle. Returns Err with the system's message when the entry cannot be read, so the page can say so instead of showing "off".
#[tauri::command]
pub fn get_autostart(app: AppHandle) -> Result<bool, String> {
    app.autolaunch().is_enabled().map_err(|e| e.to_string())
}

/// Turns Launch at Login on or off. Inputs: the app handle and the wanted state. Returns Err with the system's message on failure.
#[tauri::command]
pub fn set_autostart(app: AppHandle, on: bool) -> Result<(), String> {
    let launch = app.autolaunch();
    (if on { launch.enable() } else { launch.disable() }).map_err(|e| e.to_string())
}

/// First run (`onboarded` unset): in a release build only (a dev build must not register its own exe in the Run key), turn Launch at Login on, once. The marker is a file, not a setting, so that a user who turns Launch at Login off before finishing onboarding is not switched back on. Input: the app handle; a failure is shown to the user.
pub fn enable_autostart_once(app: &AppHandle) {
    if cfg!(debug_assertions) || super::get("onboarded").is_some() {
        return;
    }
    let Ok(dir) = app.path().app_data_dir() else { return };
    let marker = dir.join("autostart.seeded");
    if marker.exists() || std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(&marker, b"")).is_err() {
        return;
    }
    if let Err(e) = app.autolaunch().enable() {
        crate::status::report(app, &format!("Could not turn on Launch at Login: {e}. Turn it on in Settings."));
    }
}
