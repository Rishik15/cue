//! File: selection.rs
//! Purpose: The selection shortcut: copy the selected text and open the palette, a list of every action that applies to it with the result beside each; what the user picks is pasted back by the preview window.
//! Contents: on_press — shortcut pressed, run the flow on its own thread (one at a time); cancel — Escape drops a flow still running; run — copy the selection, build the rows, open the palette (cancellation is checked before the copy, after it and right before the palette opens); notice — log and show a pill line; selected_text — the copied selection or why there is none.
//! Escape bumps `CURRENT`, so a cancelled flow opens nothing.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use tauri::AppHandle;

use crate::actions::Source;
use crate::platform::{self, overlay, Capture, Target};
use crate::prefs::Prefs;
use crate::rules::{self, model::Trigger};
use crate::{palette, preview, status, text};

/// Longest selection an action will work on: a runaway select-all in a huge document must not stall the flow or fill memory.
const MAX_SELECTION_CHARS: usize = 200_000;

/// Counts flows and cancellations: a flow whose id is no longer current was cancelled.
static CURRENT: AtomicU64 = AtomicU64::new(0);
/// A flow is running. A second press meanwhile is ignored: two flows would both send Ctrl+C and fight over the clipboard.
static BUSY: AtomicBool = AtomicBool::new(false);

/// A selection is being read or the palette is being built.
pub fn is_busy() -> bool {
    BUSY.load(Ordering::Acquire)
}

/// The selection shortcut was pressed. A card that is up gives way. Dictation owns the overlay and the keyboard while it records, so copying then would put Ctrl+C into the middle of it; an open palette or preview has the focus, so copying would read from it, not from the user's app.
/// The flow runs on its own thread, because copying waits for the app and for the shortcut's keys to be released, which must not hold up the hotkey thread.
pub fn on_press(app: &AppHandle) {
    crate::dictionary::cancel();
    if crate::busy::busy(app) || BUSY.swap(true, Ordering::AcqRel) {
        return;
    }
    let id = CURRENT.fetch_add(1, Ordering::AcqRel) + 1;
    let handle = app.clone();
    let started = crate::guard::spawn_or_report(app, "selection", 512 * 1024, "The selection flow", move || {
        let _release = crate::guard::Release(&BUSY);
        run(&handle, id);
    });
    if !started {
        BUSY.store(false, Ordering::Release);
    }
}

/// Escape: drops a flow that is still running.
pub fn cancel() {
    CURRENT.fetch_add(1, Ordering::AcqRel);
}

/// Logs the line and shows it on the pill.
fn notice(text: &str) {
    crate::log::note(&format!("selection: {text}"));
    let prefs = Prefs::load();
    overlay::notice(text, prefs.overlay_top, prefs.theme());
}

/// One flow. The preview window is warmed first thing, so it can open the moment the press is seen: built hidden and unfocusable (it must never take the focus, or Ctrl+C would go to it instead of the user's app) and shown at once with a "Reading the selection" state. Focus is handed back to the app as a second guard.
/// A saved action that uses the text model loads it while the selection is copied.
fn run(app: &AppHandle, id: u64) {
    let token = platform::capture_target();
    let target = token.window;
    preview::warm(app);
    if platform::foreground_window() != target {
        platform::focus_window(target);
    }
    if let Some(task) = rules::model_task_for(Trigger::Selection) {
        text::prepare(task);
    }
    let cancelled = || CURRENT.load(Ordering::Acquire) != id;
    if cancelled() {
        return preview::dismiss(app);
    }
    let Some((text, html, token)) = selected_text_for(app, token, &cancelled) else { return preview::dismiss(app) };
    let app_name = platform::foreground_app();
    let mut items = palette::build(&text, &app_name, html.as_deref(), Source::Selection);
    if items.is_empty() {
        preview::dismiss(app);
        return notice("No action changes this text");
    }
    if cancelled() {
        return preview::dismiss(app);
    }
    items.insert(0, palette::selected_row(&text));
    preview::open_palette(app, (text, app_name), items, token);
}

/// The selected text and its HTML (when the program offered any), or None after saying why there is none. The token is taken here, for callers that have none of their own.
pub(crate) fn selected_text(app: &AppHandle) -> Option<(String, Option<String>)> {
    selected_text_for(app, platform::capture_target(), &|| false).map(|(text, html, _)| (text, html))
}

/// `selected_text` for the place named by `token`, taken before anything slow happened; the third value is the token of the field the text was copied from, which delivery checks.
fn selected_text_for(app: &AppHandle, token: Target, cancelled: &dyn Fn() -> bool) -> Option<(String, Option<String>, Target)> {
    match platform::capture(token, cancelled) {
        Capture::Text(copied) if copied.text.chars().count() > MAX_SELECTION_CHARS => notice("Selection is too long"),
        Capture::Text(copied) => match crate::sanitize::clean_selection(&copied.text) {
            clean if clean.is_empty() => notice("Nothing selected, or Cue cannot see it"),
            clean => {
                if !copied.restored {
                    status::report(app, "Cue could not put your previous clipboard back after reading the selection.");
                }
                return Some((clean, copied.html, copied.target));
            }
        },
        Capture::Empty => notice("Nothing selected, or Cue cannot see it"),
        Capture::Unknown => notice("Cue could not tell what was selected, so it used nothing"),
        Capture::Cancelled => {}
        Capture::Refused(why) => status::report(app, &why),
    }
    None
}
