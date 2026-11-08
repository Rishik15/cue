//! File: status.rs
//! Purpose: Surface problems to the user without a window: a short message pill on the overlay right away, and the tray tooltip until things work again.
//! Contents: report / report_as — a shortcut or dictation problem (tooltip and pill on screen); report_in_app — a problem with something started in the settings window (tooltip and a `cue-toast` event, or the pill when that window is closed); clear — back to normal.

use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager};

use crate::platform::overlay;
use crate::prefs::Prefs;
use crate::tray::TRAY_ID;

const IDLE: &str = "Cue";

static LAST: Mutex<String> = Mutex::new(String::new());

/// Keeps the text now shown in the tooltip. Input: the text, empty for none.
fn remember(text: &str) {
    *LAST.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = text.to_string();
}

/// Windows keeps only the first 127 characters of a tray tooltip.
const TOOLTIP_CHARS: usize = 127;

/// Sets the tray tooltip, cut to what Windows keeps. Inputs: app handle and text.
fn set_tooltip(app: &AppHandle, text: &str) {
    let shortened: String;
    let text = if text.chars().count() > TOOLTIP_CHARS {
        shortened = text.chars().take(TOOLTIP_CHARS - 1).chain(std::iter::once('…')).collect();
        &shortened
    } else {
        text
    };
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_tooltip(Some(text));
    }
}

/// The pill has room for one short line: the problem's first sentence.
fn headline(problem: &str) -> &str {
    problem.split(". ").next().unwrap_or(problem).trim_end_matches('.')
}

/// Shows a problem on the pill and in the tooltip, with the pill wording taken from its first sentence. Inputs: app handle and the problem text.
pub fn report(app: &AppHandle, problem: &str) {
    report_as(app, problem, headline(problem));
}

/// Logs a problem (bounded by the log) and shows it in the tooltip. Inputs: app handle and the problem text.
fn record(app: &AppHandle, problem: &str) {
    eprintln!("cue: {problem}");
    crate::log::note(&format!("problem: {problem}"));
    remember(problem);
    set_tooltip(app, &format!("Cue: {problem}"));
}

/// Shows a notice on the pill. Inputs: app handle and the short line.
fn show_pill(app: &AppHandle, pill: &str) {
    let (top, theme) = Prefs::notice(app);
    overlay::notice(pill, top, theme);
}

/// A problem with a shortcut or the dictation flow: always the pill on screen, whatever window is open.
/// Like `report`, with wording of its own for the pill.
pub fn report_as(app: &AppHandle, problem: &str, pill: &str) {
    record(app, problem);
    show_pill(app, pill);
}

/// A problem with something the user started in the settings window (a download): a toast there, or the pill when the window is closed.
pub fn report_in_app(app: &AppHandle, problem: &str) {
    record(app, problem);
    if app.get_webview_window("settings").is_some_and(|w| w.is_visible().unwrap_or(false)) {
        let _ = app.emit("cue-toast", problem);
    } else {
        show_pill(app, headline(problem));
    }
}

/// Returns the tooltip to normal. Input: app handle. Does nothing when no problem is shown, so the hotkey thread skips the round trip to the main thread.
pub fn clear(app: &AppHandle) {
    if LAST.lock().unwrap_or_else(std::sync::PoisonError::into_inner).is_empty() {
        return;
    }
    remember("");
    set_tooltip(app, IDLE);
}
