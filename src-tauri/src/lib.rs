//! Purpose: Cue application core entry; wires plugins, state, tray, and commands.
//! Win32-specific work lives in `platform`; macOS gets its twin there later.
//! Contents: `run` — builds the app, keeps it alive in the tray when the settings window closes; `run_speech_worker` — the engine process entry; `insert_test` — debug-only text insertion for `scripts/insert-check.ps1`.

mod actions;
mod audio;
mod data;
mod dictation;
mod mem;
mod platform;
mod prefs;
mod preview;
mod rules;
mod selection;
mod settings;
mod shortcut;
mod speech;
mod status;
mod tray;

use tauri::{RunEvent, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;

/// Entry for `cue --speech-worker`: the speech engine process, no UI. Returns the process exit code.
pub fn run_speech_worker() -> i32 {
    speech::worker::main()
}

/// Entry for `cue --insert-test <text> [paste]` (debug builds): inserts the text into the focused window like a finished dictation, then exits.
#[cfg(debug_assertions)]
pub fn insert_test(text: &str, always_paste: bool) -> i32 {
    let options = platform::InsertOptions { always_paste, ..Default::default() };
    match platform::insert(text, &options) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("cue: {e}");
            1
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // First plugin: a second launch must not start a second keyboard hook; it just opens settings.
        .plugin(tauri_plugin_single_instance::init(|app, _, _| tray::open_settings(app)))
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec!["--background"])))
        .setup(|app| {
            rules::init(app.handle());
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
            actions::list_actions,
            audio::list_microphones,
            preview::preview_get,
            preview::preview_apply,
            preview::preview_copy,
            preview::preview_close,
            rules::get_rules,
            rules::save_rules,
            rules::test_rules,
            rules::rule_log,
            speech::models::list_models,
            speech::models::download_model,
            speech::models::cancel_download,
            speech::models::delete_model,
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
