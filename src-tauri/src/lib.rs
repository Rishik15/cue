//! Purpose: Cue application core entry; wires plugins, state, tray, and commands.
//! Win32-specific work lives in `platform`; macOS gets its twin there later.
//! Contents: `run` — builds the app, keeps it alive in the tray when the settings window closes; `run_speech_worker` — the engine process entry; `insert_test` — debug-only text insertion for `scripts/insert-check.ps1`.

mod actions;
mod audio;
mod busy;
mod data;
mod dictation;
mod dictionary;
mod grab;
mod guard;
mod log;
mod mem;
mod notes;
mod palette;
mod platform;
mod prefs;
mod preview;
mod rules;
mod sanitize;
mod selection;
mod settings;
mod shortcut;
mod snippets;
mod speech;
mod stats;
mod status;
mod text;
mod tray;
mod update;
mod weblinks;

#[cfg(test)]
mod commands_test;

use tauri::{AppHandle, RunEvent, Window, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;

/// First thing in every process: only the app folder and System32 are searched for DLLs, so a planted DLL in the working directory is never loaded.
pub fn harden_process() {
    platform::harden_process();
}

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

fn setup(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    log::init(app);
    log::note(&format!("started, version {}", env!("CARGO_PKG_VERSION")));
    guard::install(app); // first: a panic in anything below is logged, not lost
    let damaged = settings::set_aside_damaged(app); // before anything reads the settings
    text::init(app);
    dictation::init(app)?;
    platform::card::init()?;
    start_snippet_expansion(app);
    tray::init(app)?;
    if damaged {
        status::report(app, "Your settings file was damaged, so Cue set it aside (settings.damaged.json) and started with the defaults.");
    }
    rules::init(app); // after the tray, so a broken rules.json can show in its tooltip
    if let Err(e) = shortcut::init(app) {
        // The tray and settings still work without the hook: say so and carry on.
        status::report(app, &format!("Cue's keyboard shortcuts could not start: {e}. Restart Cue to try again."));
    }
    settings::enable_autostart_once(app);
    update::init(app);
    update::announce_if_updated(app);
    // Launched at login (or "Open Settings on Launch" off): stay in the tray, no window. The first run always opens it for the walkthrough, even at login.
    let first_run = !settings::get_bool(app, "onboarded", false);
    let background = std::env::args().any(|a| a == "--background");
    if first_run || (!background && settings::get_bool(app, "open_on_launch", false)) {
        tray::open_settings(app);
    }
    Ok(())
}

/// Typed triggers work from the start: read the snippets on a worker thread (the database waits on the disk) and start the expander if there is anything to expand.
fn start_snippet_expansion(app: &tauri::AppHandle) {
    let handle = app.clone();
    if let Err(e) = guard::spawn("snippets-sync", 256 * 1024, move || {
        snippets::sync_expansion(&handle);
        mem::trim_soon(); // startup touched pages it will not need again
    }) {
        eprintln!("cue: could not start snippet expansion: {e}");
    }
}

/// Closing a window frees its webview; hand the pages back to Windows, and drop a preview's pending text.
fn on_window_event(window: &Window, event: &WindowEvent) {
    tray::note_window_event(window.label(), event);
    if let (WindowEvent::Focused(focused), label) = (event, window.label()) {
        if label == "preview" {
            preview::on_focus(tauri::Manager::app_handle(window), *focused);
        }
    }
    if matches!(event, WindowEvent::Destroyed) {
        match window.label() {
            "preview" => preview::forget(tauri::Manager::app_handle(window)),
            "ask" => snippets::ask::cancel(tauri::Manager::app_handle(window)),
            "notes" => platform::stop_clipboard_watch(), // closed by any means: nothing is collecting copies any more
            "settings" => shortcut::cancel_recording(tauri::Manager::app_handle(window)),
            _ => {}
        }
        mem::trim_soon();
    }
}

fn with_plugins(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    builder
        // First plugin: a second launch must not start a second keyboard hook; it just opens settings.
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            // Off the event loop's thread: opening waits for a closing settings window, which only that thread can finish.
            let app = app.clone();
            let _ = guard::spawn("open-settings", 512 * 1024, move || tray::open_settings(&app));
        }))
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec!["--background"])))
}

/// Every command the windows may call; keep in step with `COMMANDS` in `build.rs`.
fn handler() -> impl Fn(tauri::ipc::Invoke) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        shortcut::get_shortcut,
        shortcut::shortcut_record,
        shortcut::reset_shortcut,
        settings::get_settings,
        settings::get_theme,
        settings::set_setting,
        settings::get_autostart,
        settings::set_autostart,
        data::remove_app_data,
        weblinks::open_link,
        tray::take_start_page,
        update::check_update,
        update::update_status,
        update::install_update,
        actions::list_actions,
        audio::list_microphones,
        preview::preview_get,
        preview::preview_apply,
        preview::preview_close,
        palette::preview_run,
        grab::list_ocr_languages,
        snippets::list_apps,
        snippets::list_snippets,
        snippets::save_snippet,
        snippets::delete_snippet,
        snippets::import_snippets,
        snippets::export_snippets,
        snippets::add_starter_snippets,
        snippets::snippet_preview,
        snippets::ask::ask_get,
        snippets::ask::ask_submit,
        notes::note,
        snippets::save_folder,
        snippets::delete_folder,
        snippets::read_clipboard_text,
        dictionary::saved::list_words,
        dictionary::saved::delete_word,
        rules::get_rules,
        rules::save_rules,
        rules::test_rules,
        rules::rule_log,
        stats::get_stats,
        speech::models::list_models,
        speech::models::download_model,
        speech::models::cancel_download,
        speech::models::delete_model,
    ]
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    with_plugins(tauri::Builder::default())
        .setup(|app| setup(app.handle()))
        .on_window_event(on_window_event)
        .invoke_handler(handler())
        .build(tauri::generate_context!())
        .expect("error while building Cue")
        .run(|_, event| {
            // Closing the last window must not quit; only the tray's Quit does (explicit exit code).
            if let RunEvent::ExitRequested { api, code: None, .. } = event {
                api.prevent_exit();
            }
        });
}
