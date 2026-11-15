//! Purpose: The preview window: after dictation it shows a finished result for review; for a screen grab it is the palette, a list of actions with the result beside each. It pastes or drops what the user chose, and delivers what the selection strip pastes.
//! A webview created when needed and destroyed when closed, so it costs nothing while unused.
//! Contents: show — a finished dictation result; open_palette_copy — a screen grab's palette (copies); deliver — a strip result into its field; warm — build the window hidden, shown by the page, and by `take_focus` once the text is copied (`window`); context — the text the window holds and its program; dismiss — close without applying; on_focus — close the palette when the user clicks away; forget — drop the pending text when the window is gone; preview_get / preview_apply / preview_close — commands for the page.

use std::sync::atomic::Ordering;
use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::palette::{self, Item};
use crate::platform::Target;
use crate::rules::{self, model::Output};
use crate::status;

use deliver::{copy, copy_dictated, copy_with_notice, notice, paste};
use window::{take_focus, HAD_FOCUS, WINDOW};

mod deliver;
mod window;

pub(crate) use window::native_handle;
pub use window::warm;

/// What the window holds: the text, a dictation result or the grab palette's rows, where the result goes and how.
struct Pending {
    original: String,
    /// A finished dictation result; None for the palette, whose results come per row.
    result: Option<String>,
    /// Name of the action that made the dictation result.
    via: String,
    /// The palette's rows (a selection); None for a dictation result.
    items: Option<Vec<Item>>,
    /// The program the text came from (the foreground app at the press), for saved actions scoped to programs and for the run log.
    app: String,
    /// The window the text came from; Apply brings it back to the front before inserting.
    target: Target,
    /// Dictated text exists nowhere else, so closing the window copies it instead of dropping it.
    keep: bool,
    /// Copy instead of paste (a screen grab has no app to paste into).
    copy: bool,
    /// The session this text belongs to (a palette's, or one taken for a dictation result): requests and the apply carry it, so a stale one never touches this text.
    session: u64,
}

static PENDING: Mutex<Option<Pending>> = Mutex::new(None);
#[cfg(test)]
mod tests;
/// What the page is given to show.
#[derive(Serialize)]
pub struct View {
    original: String,
    result: Option<String>,
    via: String,
    items: Option<Vec<Item>>,
    /// "copy" for a screen grab, otherwise "paste".
    mode: &'static str,
    /// The session the page sends back with Paste (and the palette with its requests).
    session: u64,
}

/// The window never showed itself: close it, rescue any dictated text, and say so.
fn stuck(app: &AppHandle) {
    let dropped = PENDING.lock().unwrap().take();
    rescue(app, dropped);
    dismiss(app);
    status::report(app, "The preview window did not open and was closed.");
}

/// Stores what the window shows, builds it if needed and lets it take the keyboard: the text is known, so Ctrl+C can no longer go to it by mistake.
fn publish(app: &AppHandle, pending: Pending) {
    let replaced = PENDING.lock().unwrap().replace(pending);
    rescue(app, replaced); // a dictated text still open is never overwritten silently
    warm(app);
    if app.get_webview_window(WINDOW).is_none() {
        // The window could not be built: the text has nowhere to be shown, so it goes to the clipboard.
        let stranded = PENDING.lock().unwrap().take();
        return rescue(app, stranded);
    }
    let _ = app.emit_to(WINDOW, "preview-changed", ());
    take_focus(app);
}

/// Dictated text exists nowhere else: when its entry is dropped without being applied, it goes to the clipboard.
fn rescue(app: &AppHandle, dropped: Option<Pending>) {
    if let Some(Pending { keep: true, result: Some(text), .. }) = dropped {
        copy_dictated(app, &text, "A dictated text that was still open was copied to the clipboard. Paste it with Ctrl+V.");
    }
}

/// A finished dictation result at once (the text is ready before any window exists). Closing the window copies it, so it is never lost.
pub fn show(app: &AppHandle, original: String, result: String, via: String, target: Target) {
    let session = palette::open();
    publish(
        app,
        Pending { original, result: Some(result), via, items: None, app: String::new(), target, keep: true, copy: false, session },
    );
}

/// The palette for a screen grab: Enter copies the chosen text.
pub fn open_palette_copy(app: &AppHandle, (original, program): (String, String), items: Vec<Item>) {
    let session = palette::open();
    publish(
        app,
        Pending {
            original,
            result: None,
            via: String::new(),
            items: Some(items),
            app: program,
            target: Target::default(),
            keep: false,
            copy: true,
            session,
        },
    );
}

/// The text the palette of `session` holds and the program it came from, for `preview_run`; None when that palette is not the one open, so a stale request never reads another palette's text.
pub fn context(session: u64) -> Option<(String, String)> {
    PENDING.lock().unwrap().as_ref().filter(|p| p.session == session).map(|p| (p.original.clone(), p.app.clone()))
}

/// Whether the preview window exists.
pub fn is_open(app: &AppHandle) -> bool {
    window::exists(app)
}

/// Focus moved to or from the window: the palette (a list of actions, like a launcher) closes when the user clicks away. A dictation result stays, since closing it would copy it unasked.
pub fn on_focus(app: &AppHandle, focused: bool) {
    if focused {
        return HAD_FOCUS.store(true, Ordering::Release);
    }
    let is_palette = PENDING.lock().unwrap().as_ref().is_some_and(|p| p.items.is_some());
    if is_palette && HAD_FOCUS.swap(false, Ordering::AcqRel) {
        dismiss(app);
    }
}

/// The window was closed by any means (its own buttons, Alt+F4): the text must not linger in memory, and dictated text is copied rather than lost.
pub fn forget(app: &AppHandle) {
    if window::exists(app) {
        return; // this is the destroyed event of an earlier window; the text now pending belongs to the newer one
    }
    palette::cancel();
    let dropped = PENDING.lock().unwrap().take();
    rescue(app, dropped);
}

/// Closes the window without applying anything (nothing to review after all, or the flow was cancelled). A dictated text still open goes to the clipboard first, as in `forget`: another flow closing the window must never drop what the user said.
pub fn dismiss(app: &AppHandle) {
    palette::cancel();
    let dropped = PENDING.lock().unwrap().take();
    rescue(app, dropped);
    if let Some(window) = app.get_webview_window(WINDOW) {
        let _ = window.destroy();
    }
}

/// What the window shows now, for its page; None when nothing is pending.
#[tauri::command]
pub fn preview_get() -> Option<View> {
    PENDING.lock().unwrap().as_ref().map(|p| View {
        original: p.original.clone(),
        result: p.result.clone(),
        via: p.via.clone(),
        items: p.items.clone(),
        mode: if p.copy { "copy" } else { "paste" },
        session: p.session,
    })
}

/// Whether the window now open belongs to palette session `session`: a request of another session (a delayed Apply of a closed palette) must not act on it, an empty one included.
fn is_session(session: u64) -> bool {
    PENDING.lock().unwrap().as_ref().is_some_and(|p| p.session == session)
}

/// The pending text once there is something to paste (a dictation result, or text from the palette's result box) and the request belongs to this palette session; otherwise Paste does nothing and the window stays.
fn take_applicable(has_text: bool, session: u64) -> Option<Pending> {
    let mut pending = PENDING.lock().unwrap();
    let wanted = |p: &Pending| p.session == session && (p.result.is_some() || (p.items.is_some() && has_text));
    pending.as_ref().is_some_and(wanted).then(|| pending.take()).flatten()
}

/// `text` is what the result box holds (the user may have edited it); `id` is the palette row it came from. A saved action whose output is Copy copies instead of pasting.
#[tauri::command]
pub fn preview_apply(app: AppHandle, text: Option<String>, id: Option<String>, session: Option<u64>) {
    if text.as_deref() == Some("") {
        if is_session(session.unwrap_or(0)) {
            refuse_empty(&app);
        }
        return;
    }
    let Some(p) = take_applicable(text.is_some(), session.unwrap_or(0)) else { return };
    let Some(result) = text.or_else(|| p.result.clone()) else { return };
    dismiss(&app);
    if p.copy {
        return copy_with_notice(&app, &result);
    }
    deliver_as(&app, id.as_deref(), &p.app, p.target, result, p.keep);
}

/// A result the user chose from the selection strip: delivered like the palette's (`deliver_as`). Input: the app, the row id, the program the text came from, the field token and the text.
pub fn deliver(app: &AppHandle, id: &str, program: &str, target: Target, text: String) {
    deliver_as(app, Some(id), program, target, text, false);
}

/// Delivers a chosen result: a saved action's run is recorded, its result copied when its output is Copy, otherwise pasted into `target` (copied when the field changed, see `paste`). `keep` marks dictated text, which is never lost. Input: the app, the row id, the program, the field token, the text and `keep`.
fn deliver_as(app: &AppHandle, id: Option<&str>, program: &str, target: Target, result: String, keep: bool) {
    let tail = id.and_then(|i| i.strip_prefix("rule:"));
    let chosen = tail.and_then(rules::choose);
    if tail.is_some() && chosen.is_none() {
        return copy(app, &result, "Your saved actions changed while this was open, so the result was copied instead of pasted.");
    }
    if let Some(c) = &chosen {
        c.record(program);
    }
    if chosen.is_some_and(|c| c.output() == Output::Copy) {
        return if keep {
            copy_dictated(app, &result, "The result was copied to the clipboard.")
        } else {
            copy(app, &result, "The result was copied to the clipboard.")
        };
    }
    paste(app, target, result, keep);
}

/// An empty result is never pasted: the window closes with a notice, and dictated text is copied so it is not lost.
fn refuse_empty(app: &AppHandle) {
    let saved = PENDING.lock().unwrap().take().filter(|p| p.keep).and_then(|p| p.result);
    if let Some(text) = saved {
        copy_dictated(app, &text, "The dictated text was copied to the clipboard. Paste it with Ctrl+V.");
    }
    dismiss(app);
    notice("Error pasting empty text");
}

/// Cancel: also stops the selection flow, so a result that arrives later opens nothing.
#[tauri::command]
pub fn preview_close(app: AppHandle, text: Option<String>) {
    // Dictated text is not in any document, so closing the window must not lose it (the box may hold the user's edits).
    let kept = PENDING.lock().unwrap().take().filter(|p| p.keep).map(|p| p.result);
    if let Some(saved) = kept.and_then(|result| text.filter(|t| !t.is_empty()).or(result)) {
        copy_dictated(&app, &saved, "The dictated text was copied to the clipboard. Paste it with Ctrl+V.");
    }
    crate::selection::cancel();
    dismiss(&app);
}
