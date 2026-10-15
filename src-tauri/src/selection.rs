//! Purpose: The selection shortcut: copy the selected text and open the palette, a list of every action that applies to it with the result beside each; what the user picks is pasted back by the preview window.
//! Contents: on_press — shortcut pressed, run the flow on its own thread (one at a time); cancel — Escape drops a flow still running; run — copy the selection, build the rows, open the palette.
//! Escape bumps `CURRENT`, so a cancelled flow opens nothing.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use tauri::AppHandle;

use crate::actions::Source;
use crate::platform::{self, overlay};
use crate::prefs::Prefs;
use crate::rules::{self, model::Trigger};
use crate::{palette, preview, status, text};

/// Longest selection an action will work on: a runaway select-all in a huge document must not stall the flow or fill memory.
const MAX_SELECTION_CHARS: usize = 200_000;

static CURRENT: AtomicU64 = AtomicU64::new(0);
/// A flow is running. A second press meanwhile is ignored: two flows would both send Ctrl+C and fight over the clipboard.
static BUSY: AtomicBool = AtomicBool::new(false);

pub fn on_press(app: &AppHandle) {
    // Dictation owns the overlay and the keyboard while it records; copying now would also put Ctrl+C into the middle of it. An open palette or
    // preview has the focus, so copying would read from it, not from the user's app.
    if crate::dictation::is_recording() || preview::is_open(app) || BUSY.swap(true, Ordering::AcqRel) {
        return;
    }
    let id = CURRENT.fetch_add(1, Ordering::AcqRel) + 1;
    let app = app.clone();
    // Own thread: copying waits for the app and for the shortcut's keys to be released, which must not hold up the hotkey thread.
    let spawned = crate::guard::spawn("selection", 512 * 1024, move || {
        let _release = crate::guard::Release(&BUSY);
        run(&app, id);
    });
    if let Err(e) = spawned {
        BUSY.store(false, Ordering::Release);
        eprintln!("cue: could not start the selection flow: {e}");
    }
}

pub fn cancel() {
    CURRENT.fetch_add(1, Ordering::AcqRel);
}

fn notice(app: &AppHandle, text: &str) {
    let prefs = Prefs::load(app);
    overlay::notice(text, prefs.overlay_top, prefs.theme());
}

fn run(app: &AppHandle, id: u64) {
    let target = platform::foreground_window();
    // First thing, so it can open the moment the press is seen: built hidden and unfocusable (it must never take the focus, or Ctrl+C would go to
    // it instead of the user's app) and shown at once with a "Reading the selection" state. Focus is handed back to the app as a second guard.
    preview::warm(app);
    if platform::foreground_window() != target {
        platform::focus_window(target);
    }
    if let Some(task) = rules::model_task_for(Trigger::Selection) {
        text::prepare(task); // a saved action uses the text model: load it while the selection is copied
    }
    let Some((text, html)) = selected_text(app) else { return preview::dismiss(app) };
    if CURRENT.load(Ordering::Acquire) != id {
        return preview::dismiss(app); // cancelled while copying
    }
    let app_name = platform::foreground_app();
    let items = palette::build(&text, &app_name, html.as_deref(), Source::Selection);
    if items.is_empty() {
        preview::dismiss(app);
        return notice(app, "No action changes this text");
    }
    preview::open_palette(app, (text, app_name), items, target);
}

/// The selected text and its HTML (when the program offered any), or None after saying why there is none.
fn selected_text(app: &AppHandle) -> Option<(String, Option<String>)> {
    match platform::copy_selection() {
        Ok(Some(copied)) if copied.text.chars().count() > MAX_SELECTION_CHARS => notice(app, "Selection is too long"),
        Ok(Some(copied)) => match crate::sanitize::clean_selection(&copied.text) {
            clean if clean.is_empty() => notice(app, "Select some text first"),
            clean => return Some((clean, copied.html)),
        },
        Ok(None) => notice(app, "Select some text first"),
        Err(e) => status::report(app, &e),
    }
    None
}
