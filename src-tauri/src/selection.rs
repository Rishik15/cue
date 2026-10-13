//! Purpose: The selection shortcut: copy the selected text, then run the Selection action that matches it (or let the user pick one when several do), and put the result back in its place.
//! Contents: on_press — shortcut pressed, run the flow on its own thread (one at a time); chosen — an action picked in the window; cancel — Escape drops a flow still running; run — copy the selection, then run the one matching action or offer the list; apply — run one action; finish — deliver the result.
//! Escape bumps `CURRENT`, so a cancelled flow never writes anything.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use tauri::AppHandle;

use crate::platform::{self, overlay, InsertOptions};
use crate::prefs::Prefs;
use crate::preview;
use crate::rules::model::{Output, Trigger};
use crate::rules::{self, Outcome};
use crate::status;
use crate::text;

/// Longest selection a rule will work on: a runaway select-all in a huge document must not stall the flow or fill memory.
const MAX_SELECTION_CHARS: usize = 200_000;

static CURRENT: AtomicU64 = AtomicU64::new(0);
/// A flow is running. A second press meanwhile is ignored: two flows would both send Ctrl+C and fight over the clipboard.
static BUSY: AtomicBool = AtomicBool::new(false);

pub fn on_press(app: &AppHandle) {
    // Dictation owns the overlay and the keyboard while it records; copying now would also put Ctrl+C into the middle of it. An open picker or
    // preview has the focus, so copying would read from it, not from the user's app.
    if crate::dictation::is_recording() || preview::is_open(app) {
        return;
    }
    start(app, run);
}

/// The user picked an action in the window: run it on the text the window holds.
pub fn chosen(app: &AppHandle, text: String, target: isize, at: usize) {
    start(app, move |app, id| apply(app, id, target, text, at));
}

/// Runs `flow` on its own thread, one at a time: a second press meanwhile is ignored, since two flows would both send Ctrl+C and fight over the clipboard.
fn start(app: &AppHandle, flow: impl FnOnce(&AppHandle, u64) + Send + 'static) {
    if BUSY.swap(true, Ordering::AcqRel) {
        return;
    }
    let id = CURRENT.fetch_add(1, Ordering::AcqRel) + 1;
    let app = app.clone();
    // Own thread: copying waits for the app and for the shortcut's keys to be released, which must not hold up the hotkey thread.
    let spawned = std::thread::Builder::new().name("selection".into()).stack_size(512 * 1024).spawn(move || {
        flow(&app, id);
        BUSY.store(false, Ordering::Release);
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
    if !rules::has_rules_for(Trigger::Selection) {
        // Nothing to run: say how to set it up, without touching the user's clipboard or selection.
        return notice(app, "Add a Selection action");
    }
    let target = platform::foreground_window();
    if let Some(task) = rules::model_task_for(Trigger::Selection) {
        text::prepare(task); // loads and warms the text model while the selection is copied
    }
    let Some(text) = selected_text(app) else { return };
    // The window is only built after the copy: a new window can take the focus, and Ctrl+C would then go to it instead of the user's app.
    let mut found = rules::matching(Trigger::Selection, &text);
    match found.len() {
        0 => notice(app, "No action matches this text"),
        1 => apply(app, id, target, text, found.remove(0).at),
        _ => preview::choose(app, text, found, target),
    }
}

/// Runs the action at `at` on `text`. A preview opens at once with a working state (the model may take seconds) and fills in when done.
fn apply(app: &AppHandle, id: u64, target: isize, text: String, at: usize) {
    let Some((via, output)) = rules::pick(at) else { return preview::dismiss(app) };
    if output == Output::Preview {
        preview::working(app, text.clone(), via, target);
    } else {
        preview::dismiss(app);
    }
    let out = rules::run_one(at, &text);
    if CURRENT.load(Ordering::Acquire) != id {
        return preview::dismiss(app); // cancelled, or a newer press took over
    }
    let Some(out) = out.filter(|o| !o.matched.is_empty()) else {
        preview::dismiss(app);
        return notice(app, "This action no longer matches");
    };
    if out.text == text && out.output != Output::Copy {
        preview::dismiss(app);
        return notice(app, "Nothing to change");
    }
    if out.output == Output::Preview {
        return preview::finish(app, out.text);
    }
    preview::dismiss(app);
    finish(app, target, out);
}

/// The selected text, or None after saying why there is none.
fn selected_text(app: &AppHandle) -> Option<String> {
    match platform::copy_selection() {
        Ok(Some(text)) if text.chars().count() > MAX_SELECTION_CHARS => notice(app, "Selection is too long"),
        Ok(Some(text)) => return Some(text),
        Ok(None) => notice(app, "Select some text first"),
        Err(e) => status::report(app, &e),
    }
    None
}

/// Replaces the selection, unless focus moved: then the result goes to the clipboard so it never lands in the wrong app.
fn finish(app: &AppHandle, target: isize, out: Outcome) {
    let moved = target != 0 && platform::foreground_window() != target;
    if out.output == Output::Copy || moved {
        match platform::copy_text(&out.text) {
            Ok(()) if out.output == Output::Copy => notice(app, "Copied"),
            Ok(()) => status::report(app, "You switched windows, so the result was copied to the clipboard."),
            Err(e) => status::report(app, &format!("Could not copy the result: {e}")),
        }
        return;
    }
    let prefs = Prefs::load(app);
    let options = InsertOptions { always_paste: prefs.always_paste, restore_clipboard: prefs.restore_clipboard };
    if let Err(e) = platform::insert(&out.text, &options) {
        status::report(app, &e.to_string());
    }
}
