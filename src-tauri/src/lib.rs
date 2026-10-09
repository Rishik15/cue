//! Purpose: Cue application core entry; wires plugins, state, tray, and commands.
//! Win32-specific work lives in `platform`; macOS gets its twin there later.
//! Contents: `run` — builds the app, keeps it alive in the tray when the settings window closes.

mod audio;
mod data;
mod dictation;
mod mem;
mod platform;
mod prefs;
mod settings;
mod shortcut;
mod status;
mod tray;

use tauri::{RunEvent, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // First plugin: a second launch must not start a second keyboard hook; it just opens settings.
        .plugin(tauri_plugin_single_instance::init(|app, _, _| tray::open_settings(app)))
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec!["--background"])))
        .setup(|app| {
            dictation::init(app.handle())?;
            shortcut::init(app.handle())?;
            tray::init(app.handle())?;
            // Launched at login (or "Open Settings on Launch" off): stay in the tray, no window.
            if !std::env::args().any(|a| a == "--background") && settings::get_bool(app.handle(), "open_on_launch", true) {
                tray::open_settings(app.handle());
            }
            Ok(())
        })
        // Closing the settings window frees its webview; hand the pages back to Windows.
        .on_window_event(|_, event| {
            if matches!(event, WindowEvent::Destroyed) {
                mem::trim_soon();
            }
        })
        .invoke_handler(tauri::generate_handler![
            shortcut::get_shortcut,
            shortcut::shortcut_record,
            shortcut::reset_shortcut,
            settings::get_settings,
            settings::set_setting,
            settings::get_autostart,
            settings::set_autostart,
            data::remove_app_data,
            audio::list_microphones,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Cue")
        .run(|_, event| {
            // Closing the last window must not quit; only the tray's Quit does (explicit exit code).
            if let RunEvent::ExitRequested { api, code: None, .. } = event {
                api.prevent_exit();
            }
        });
}
