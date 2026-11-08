//! File: status.rs
//! Purpose: Surface problems to the user without a window: a short message pill on the overlay right away, and the tray tooltip until things work again.
//! Contents: report / report_as — a shortcut or dictation problem (tooltip and pill on screen); report_in_app — a problem with something started in the settings window (tooltip and a `cue-toast` event, or the pill when that window is closed); clear — back to normal.

use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager};

use crate::platform::overlay;
use crate::prefs::Prefs;
use crate::tray::TRAY_ID;

const IDLE: &str = "Cue";

/// The tooltip text Cue wants (empty for none) and how many times it changed, so a slower writer can see that a newer change overtook it.
#[derive(Default)]
struct Wanted {
    text: String,
    generation: u64,
}

static LAST: Mutex<Wanted> = Mutex::new(Wanted { text: String::new(), generation: 0 });

/// Locks the wanted tooltip state; a panic elsewhere must not stop problems from showing. Returns the guard.
fn wanted() -> std::sync::MutexGuard<'static, Wanted> {
    LAST.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Records the tooltip text now wanted. Input: the text, empty for none. Returns nothing.
fn want(text: &str) {
    let mut state = wanted();
    state.text = text.to_string();
    state.generation += 1;
}

/// Makes the tray tooltip match the latest wanted text. Input: app handle. See `converge`.
fn sync_tooltip(app: &AppHandle) {
    converge(|text| set_tooltip(app, text));
}

/// Applies the wanted tooltip text, then checks that no newer change arrived meanwhile and applies that one too, so two threads reporting and clearing together cannot leave an old tooltip behind: whichever applies last has just checked that nothing newer exists. Input: the function that sets the tooltip. Returns nothing.
fn converge(mut apply: impl FnMut(&str)) {
    loop {
        let (text, generation) = {
            let state = wanted();
            (state.text.clone(), state.generation)
        };
        apply(if text.is_empty() { IDLE } else { &text });
        if wanted().generation == generation {
            return;
        }
    }
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
    want(&format!("Cue: {problem}"));
    sync_tooltip(app);
}

/// Shows a notice on the pill. Inputs: app handle and the short line.
fn show_pill(pill: &str) {
    let (top, theme) = Prefs::notice();
    overlay::notice(pill, top, theme);
}

/// A problem with a shortcut or the dictation flow: always the pill on screen, whatever window is open.
/// Like `report`, with wording of its own for the pill.
pub fn report_as(app: &AppHandle, problem: &str, pill: &str) {
    record(app, problem);
    show_pill(pill);
}

/// A problem with something the user started in the settings window (a download): a toast there, or the pill when the window is closed.
pub fn report_in_app(app: &AppHandle, problem: &str) {
    record(app, problem);
    if app.get_webview_window("settings").is_some_and(|w| w.is_visible().unwrap_or(false)) {
        let _ = app.emit("cue-toast", problem);
    } else {
        show_pill(headline(problem));
    }
}

/// Returns the tooltip to normal. Input: app handle. Does nothing when no problem is shown, so the hotkey thread skips the round trip to the main thread.
pub fn clear(app: &AppHandle) {
    if wanted().text.is_empty() {
        return;
    }
    want("");
    sync_tooltip(app);
}

#[cfg(test)]
mod tests;
