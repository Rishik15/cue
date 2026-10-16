//! Purpose: Surface problems to the user without a window: a short message pill on the overlay right away, and the tray tooltip until things work again.
//! Contents: report / report_as — show a problem (tooltip, the `cue-status` event the settings window shows as a toast, and a pill on the screen when that window is not open); clear — back to normal;

use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager};

use crate::platform::overlay;
use crate::prefs::Prefs;
use crate::tray::TRAY_ID;

const IDLE: &str = "Cue";

static LAST: Mutex<String> = Mutex::new(String::new());

fn remember(text: &str) {
    *LAST.lock().unwrap() = text.to_string();
}

/// Windows keeps only the first 127 characters of a tray tooltip.
const TOOLTIP_CHARS: usize = 127;

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

pub fn report(app: &AppHandle, problem: &str) {
    report_as(app, problem, headline(problem));
}

/// Like `report`, with wording of its own for the pill.
pub fn report_as(app: &AppHandle, problem: &str, pill: &str) {
    eprintln!("cue: {problem}");
    crate::log::note(&format!("problem: {problem}"));
    // With the settings window open the problem shows there, as a toast (it listens for `cue-status`); the screen pill is for when no window is.
    if !app.get_webview_window("settings").is_some_and(|w| w.is_visible().unwrap_or(false)) {
        let prefs = Prefs::load(app);
        overlay::notice(pill, prefs.overlay_top, prefs.theme());
    }
    remember(problem);
    set_tooltip(app, &format!("Cue: {problem}"));
    let _ = app.emit("cue-status", problem);
}

pub fn clear(app: &AppHandle) {
    if LAST.lock().unwrap().is_empty() {
        return; // nothing shown: skip the round trip to the main thread (this runs on the hotkey thread)
    }
    remember("");
    set_tooltip(app, IDLE);
    let _ = app.emit("cue-status", "");
}
