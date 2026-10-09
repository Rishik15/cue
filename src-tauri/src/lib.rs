//! Purpose: Cue application core entry; wires plugins, state, tray, and commands.
//! Contents: `run` — builds the app, keeps it alive in the tray when the settings window closes.

mod settings;
mod shortcut;
mod tray;

use tauri::RunEvent;
use tauri_plugin_autostart::MacosLauncher;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec!["--background"])))
        .setup(|app| {
            shortcut::init(app.handle())?;
            tray::init(app.handle())?;
            // Launched at login: stay in the tray, no window.
            if !std::env::args().any(|a| a == "--background") {
                tray::open_settings(app.handle());
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            shortcut::get_shortcut,
            shortcut::shortcut_record,
            shortcut::reset_shortcut,
            settings::get_setting,
            settings::set_setting,
            settings::get_autostart,
            settings::set_autostart,
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
