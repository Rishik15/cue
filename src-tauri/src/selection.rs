//! Purpose: The selection shortcut: copy the selected text, run the Selection rules over it, and put the result back in its place.
//! Contents: on_press — shortcut pressed, run the flow on its own thread; cancel — Escape drops a flow still running; run — the flow; finish — deliver the result.
//! A press while a flow runs, or Escape, bumps `CURRENT`, so a stale flow never writes anything.

use std::sync::atomic::{AtomicU64, Ordering};

use tauri::AppHandle;

use crate::platform::{self, overlay, InsertOptions};
use crate::prefs::Prefs;
use crate::rules::model::{Output, Trigger};
use crate::rules::{self, Outcome};
use crate::status;

static CURRENT: AtomicU64 = AtomicU64::new(0);

pub fn on_press(app: &AppHandle) {
    let id = CURRENT.fetch_add(1, Ordering::AcqRel) + 1;
    let app = app.clone();
    // Own thread: copying waits for the app and for the shortcut's keys to be released, which must not hold up the hotkey thread.
    let spawned = std::thread::Builder::new().name("selection".into()).stack_size(512 * 1024).spawn(move || run(&app, id));
    if let Err(e) = spawned {
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
    let text = match platform::copy_selection() {
        Ok(Some(text)) => text,
        Ok(None) => return notice(app, "Select some text first"),
        Err(e) => return status::report(app, &e),
    };
    let out = rules::run(Trigger::Selection, &text);
    if CURRENT.load(Ordering::Acquire) != id {
        return; // cancelled, or a newer press took over
    }
    if out.matched.is_empty() {
        return notice(app, "No rule matches this text");
    }
    if out.text == text && out.output == Output::Insert {
        return notice(app, "Nothing to change");
    }
    finish(app, target, out);
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
