//! File: lib.rs
//! Purpose: Cue application core entry; wires plugins, state, tray, and commands.
//! Win32-specific work lives in `platform`; macOS gets its twin there later.
//! Contents:
//! - `run` — builds the app, keeps it alive in the tray when the settings window closes
//! - `run_speech_worker` — the engine process entry
//! - `insert_test` — debug-only text insertion for `scripts/verify/insert-check.ps1`

mod actions;
mod audio;
mod busy;
mod data;
mod dictation;
mod dictionary;
mod grab;
mod guard;
mod input;
mod log;
mod mem;
mod model_download;
mod notes;
mod palette;
mod platform;
mod prefs;
mod preview;
mod rules;
mod runtime;
mod sanitize;
mod selection;
mod settings;
mod snippets;
mod speech;
mod stats;
mod status;
mod text;
mod tray;
mod update;
mod weblinks;

use tauri::{AppHandle, Manager, RunEvent, Window, WindowEvent};
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

/// Starts every part of Cue in order, with the logger and the panic guard first so a panic in anything after them is logged and not lost, and the settings file checked before anything reads it. The text engine is set up before model downloads are recovered (recovery never starts a download), the rules after the tray so a broken rules.json can show in its tooltip, and a keyboard-shortcut failure is reported while the tray and settings keep working. Cue stays in the tray without a window when launched at login (or with "Open Settings on Launch" off); the first run always opens Settings for the walkthrough, even at login. Input: the app handle. Return: an error only when dictation or the tray cannot start.
fn setup(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    log::init(app);
    log::note(&format!("started, version {}", env!("CARGO_PKG_VERSION")));
    log::note(&platform::machine_summary());
    guard::install(app);
    let reporter = app.clone();
    platform::set_reporter(move |problem| status::report(&reporter, problem));
    let settings_notice = settings::init(app);
    text::init(app);
    model_download::recover(app);
    dictation::init(app)?;
    start_snippet_expansion(app);
    tray::init(app)?;
    if let Some(notice) = settings_notice {
        status::report(app, &notice);
    }
    rules::init(app);
    app.manage(input::Bridge::default());
    match input::start(app, &input::CATALOG) {
        Ok(running) => {
            app.manage(running);
        }
        Err(e) => status::report(app, &format!("Cue's keyboard shortcuts could not start: {e} Restart Cue to try again.")),
    }
    settings::enable_autostart_once(app);
    update::init(app);
    update::announce_if_updated(app);
    let first_run = !settings::get_bool("onboarded", false);
    let background = std::env::args().any(|a| a == "--background");
    if first_run || (!background && settings::get_bool("open_on_launch", false)) {
        tray::open_settings(app);
    }
    Ok(())
}

/// Starts typed snippet triggers: reads the snippets on a worker thread (the database waits on the disk), starts the expander if there is anything to expand, and then trims the working set because startup touched pages it will not need again. Input: the app handle; a thread that cannot start is reported to the user.
fn start_snippet_expansion(app: &tauri::AppHandle) {
    let handle = app.clone();
    guard::spawn_or_report(app, "snippets-sync", 256 * 1024, "Typed snippet triggers", move || {
        snippets::sync_expansion(&handle);
        mem::trim_soon();
    });
}

/// Reacts to window events: tells the tray, and on a close hands the pages back to Windows and drops what the window owned (a preview's pending text, an open question, the clipboard collection of the notes window, a shortcut recording of the settings window), because a window closed by any means leaves nothing using them. Input: the window and the event.
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
            "notes" => platform::stop_clipboard_watch(),
            "settings" => input::window_closed(tauri::Manager::app_handle(window)),
            _ => {}
        }
        mem::trim_soon();
    }
}

/// Adds the plugins Cue uses. The single-instance guard comes first, so a second launch never starts a second keyboard hook and only asks the first to open Settings; that open runs on its own thread because it waits for a closing settings window, which only the event loop's thread can finish. Input: the builder. Return: the builder with the plugins.
fn with_plugins(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    builder
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            let handle = app.clone();
            guard::spawn_or_report(app, "open-settings", 512 * 1024, "Settings", move || tray::open_settings(&handle));
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec!["--background"])))
}

/// Every command the windows may call; keep in step with `COMMANDS` in `build.rs`. shortcut: a flat IPC registry Tauri requires in one macro; one line per command, no boundary to split on. Limit lifts if Tauri ever takes a split list.
#[allow(clippy::too_many_lines)]
fn handler() -> impl Fn(tauri::ipc::Invoke) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        input::bridge::shortcut_catalog,
        input::bridge::shortcut_record_open,
        input::bridge::shortcut_record_cancel,
        input::bridge::shortcut_record_regions,
        input::bridge::shortcut_resnapshot,
        input::bridge::shortcut_reset,
        settings::get_settings,
        settings::get_theme,
        settings::set_setting,
        settings::autostart::get_autostart,
        settings::autostart::set_autostart,
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
        notes::breaks::break_act,
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
/// Builds and runs the app until the tray's Quit. It first disables power throttling (a tray-only process is otherwise moved to efficiency cores at low clocks, which delays hook callbacks and typing). Closing the last window does not quit; only Quit does, with an explicit exit code, and on exit the speakers are given back so a recording in progress cannot leave them muted, and downloads are shut down.
pub fn run() {
    platform::disable_power_throttling();
    with_plugins(tauri::Builder::default())
        .setup(|app| setup(app.handle()))
        .on_window_event(on_window_event)
        .invoke_handler(handler())
        .build(tauri::generate_context!())
        .expect("error while building Cue")
        .run(|_, event| match event {
            RunEvent::ExitRequested { api, code: None, .. } => api.prevent_exit(),
            RunEvent::Exit => {
                dictation::mute::release();
                model_download::shutdown();
            }
            _ => {}
        });
}
