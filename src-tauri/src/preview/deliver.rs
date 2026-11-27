//! File: preview/deliver.rs
//! Purpose: Gets a finished preview result to where it goes: pasted into the app it came from, or copied to the clipboard (a grab has no app to paste into, and any failure falls back to the clipboard so nothing is lost).
//! Contents: GONE / GONE_WAIT — the Destroyed signal a paste waits on and how long; window_gone — a preview or strip window was destroyed; wait_gone — wait for the windows to be gone; paste — return to the target app and insert the text; copy / copy_with_notice / copy_dictated — put text on the clipboard, with a note or the "Copied" pill (a dictated text whose copy fails is kept in the recovery slot); notice — show the pill with a message.

use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};

use crate::dictation::recovery;
use crate::lock::{lock, wait_timeout_while};
use crate::platform::{self, InsertOptions, Target};
use crate::prefs::Prefs;
use crate::status;

/// When a preview or strip window was last destroyed, under the lock `GONE_WAKE` waits with, so a paste wakes the moment its window is gone.
static GONE: Mutex<Option<Instant>> = Mutex::new(None);
/// Woken by `window_gone`.
static GONE_WAKE: Condvar = Condvar::new();
/// How long a paste waits for the windows to be gone before it copies instead.
const GONE_WAIT: Duration = Duration::from_secs(2);

/// A preview or strip window was destroyed (its Destroyed event): wakes a paste waiting for it.
pub fn window_gone() {
    *lock(&GONE) = Some(Instant::now());
    GONE_WAKE.notify_all();
}

/// Waits until `open` says no window is left, woken by `window_gone`. Input: the check (called under the lock, so a Destroyed event cannot slip between it and the wait) and the time limit. Return: None when a window is still open at the limit, else how long ago the window waited for was destroyed (zero when none was waited for).
pub(super) fn wait_gone(open: impl Fn() -> bool, limit: Duration) -> Option<Duration> {
    let start = Instant::now();
    let gone = wait_timeout_while(&GONE_WAKE, lock(&GONE), limit, |_| open());
    (!open()).then(|| gone.filter(|t| *t >= start).map_or(Duration::ZERO, |t| t.elapsed()))
}

/// Returns focus to `target` and inserts `result` there, on its own thread; it first waits for the preview and strip windows' Destroyed events (`dismiss` only asks for a window to be destroyed, and returning focus while it still exists lets its closing take the focus back), and copies instead when one is still there after `GONE_WAIT`: returning focus and typing wait on other programs, and the command must not hold up the window's event loop. Focus is checked again at the moment of delivery; a delivery that went through is logged ("preview: pasted") for the harnesses.
pub(super) fn paste(app: &AppHandle, target: Target, result: String, keep: bool) {
    let (handle, backup) = (app.clone(), result.clone());
    let spawned = crate::guard::spawn("preview-apply", 512 * 1024, move || {
        let open = || handle.get_webview_window(super::WINDOW).is_some() || crate::strip::is_open(&handle);
        let Some(since) = wait_gone(open, GONE_WAIT) else {
            return fallback(&handle, &result, "The window did not close in time, so the result was copied to the clipboard.", keep);
        };
        crate::log::note(&format!("preview: paste starts {} ms after the window was destroyed", since.as_millis()));
        if !platform::focus_window(target.window) {
            return fallback(&handle, &result, "Cue could not return to the app, so the result was copied to the clipboard.", keep);
        }
        let prefs = Prefs::load();
        let options = InsertOptions {
            always_paste: prefs.always_paste,
            restore_clipboard: prefs.restore_clipboard,
            target: target.window,
            token: Some(target),
            ..Default::default()
        };
        match crate::guard::contain(|| platform::insert(&result, &options)) {
            Some(Ok(())) => crate::log::note("preview: pasted"),
            Some(Err(e)) if keep => recovery::report_failure(&handle, &result, &e),
            Some(Err(e)) => status::report_as(&handle, &e.to_string(), e.pill()),
            None => fallback(&handle, &result, "Typing the result hit a problem, so it was copied to the clipboard.", keep),
        }
    });
    if let Err(e) = spawned {
        fallback(app, &backup, &format!("Could not apply the result ({e}), so it was copied to the clipboard."), keep);
    }
}

/// Copies `text` and shows `note`; a dictated text (`keep`) that cannot be copied is kept in the recovery slot.
fn fallback(app: &AppHandle, text: &str, note: &str, keep: bool) {
    if keep {
        copy_dictated(app, text, note);
    } else {
        copy(app, text, note);
    }
}

/// A grab's result goes to the clipboard, and the pill says so.
pub(super) fn copy_with_notice(app: &AppHandle, text: &str) {
    match platform::copy_text(text) {
        Ok(()) => {
            let prefs = Prefs::load();
            platform::overlay::notice("Copied", prefs.overlay_top, prefs.theme());
        }
        Err(e) => status::report(app, &format!("Could not copy the text: {e}")),
    }
}

/// Copies the text to the clipboard and shows `note` (when there is one), or reports why it could not be copied.
pub(super) fn copy(app: &AppHandle, text: &str, note: &str) {
    match platform::copy_text(text) {
        Ok(()) if !note.is_empty() => status::report(app, note),
        Ok(()) => {}
        Err(e) => status::report(app, &format!("Could not copy the result: {e}")),
    }
}

/// The native pill with a short message.
pub(super) fn notice(text: &str) {
    let prefs = Prefs::load();
    platform::overlay::notice(text, prefs.overlay_top, prefs.theme());
}

/// Copies a dictated text to the clipboard and shows `note`. Dictated text exists nowhere else, so a copy that fails keeps it in the dictation recovery slot and says so.
pub(super) fn copy_dictated(app: &AppHandle, text: &str, note: &str) {
    match platform::copy_text(text) {
        Ok(()) => {
            recovery::copied(text);
            status::report(app, note);
        }
        Err(e) => {
            let earlier = recovery::hold_note(text);
            status::report(app, &format!("Could not copy the dictated text: {e}. Cue is keeping it for this session.{earlier}"));
        }
    }
}
