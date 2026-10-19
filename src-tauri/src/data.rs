//! Purpose: "Remove all Cue data": undo everything Cue wrote outside its install folder.
//! Contents: remove_app_data — turns launch-at-login off, deletes the settings and local data directories (the web view
//! profile is locked while running, so those go after exit), and quits.

use tauri::{AppHandle, Manager};
use tauri_plugin_autostart::ManagerExt;

use crate::platform;

#[tauri::command]
pub fn remove_app_data(app: AppHandle) -> Result<(), String> {
    let launch = app.autolaunch();
    if let Err(e) = launch.disable() {
        if launch.is_enabled().unwrap_or(true) {
            return Err(format!("Could not turn off Launch at Login: {e}"));
        }
    }
    let path = app.path();
    let dirs: Vec<_> = [path.app_data_dir(), path.app_local_data_dir()].into_iter().flatten().collect();
    platform::remove_dirs_after_exit(&dirs);
    app.exit(0);
    Ok(())
}
