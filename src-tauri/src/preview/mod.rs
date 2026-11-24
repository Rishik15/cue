//! Purpose: The preview window: after dictation it shows a finished result for review; for a screen grab it is the palette, a list of actions with the result beside each. It pastes or drops what the user chose, and delivers what the selection strip pastes.
//! A webview created when needed and destroyed when closed, so it costs nothing while unused.
//! Contents: show — a finished dictation result; open_palette_copy — a screen grab's palette (copies); deliver — a strip result into its field; warm — build the window hidden, shown by `take_focus` once the text is copied (`window`); context — the text the window holds and its program; dismiss — close without applying; on_focus — close the palette when the user clicks away; forget — drop the pending text when the window is gone; preview_get / preview_apply / preview_close — commands for the page.

use crate::lock::lock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::palette::{self, Item};
use crate::platform::Target;
use crate::rules::{self, model::Output};
use crate::status;
use crate::window_guard;

use deliver::{copy, copy_dictated, copy_with_notice, notice, paste};
use window::{take_focus, FOCUS, OWNER, WINDOW};

mod deliver;
mod window;

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
/// Set while a retired window is not yet destroyed (its Destroyed cleanup not finished): a new text must not be published then, or it would reuse a window about to be destroyed or be taken by that cleanup. A flag, so retiring one window twice counts once.
static CLOSING: AtomicBool = AtomicBool::new(false);
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

/// Retires the window `ok` accepts, all under the pending lock so the check and the retirement see the same window: ends its watchdog, forgets its owner number, cancels the model run and takes the pending text; `grab` (run under the same lock) fetches whatever must close with it. Input: the check on the pending text (None when nothing is pending) and `grab`. Return: None when `ok` refused (a newer window or session is there), else the taken text and what `grab` returned.
fn retire<T>(ok: impl FnOnce(Option<&Pending>) -> bool, grab: impl FnOnce() -> T) -> Option<(Option<Pending>, T)> {
    let mut pending = lock(&PENDING);
    if !ok(pending.as_ref()) {
        return None;
    }
    window::WATCH.disarm();
    OWNER.store(0, Ordering::Release);
    palette::cancel();
    CLOSING.store(true, Ordering::Release);
    Some((pending.take(), grab()))
}

/// Ends the retirement: lets publication go on. Harmless when nothing was retired, since a window closed by the user also ends with a Destroyed event.
fn release() {
    CLOSING.store(false, Ordering::Release);
}

/// Ends a retirement: asks for the window `retire` grabbed to be destroyed; publication waits for its Destroyed event (`forget`), since the destruction only runs later on the event loop. With no window, or when the request fails, publication goes on at once. Input: the grabbed window.
fn finish(window: Option<tauri::WebviewWindow>) {
    if window.is_none_or(|w| w.destroy().is_err()) {
        release();
    }
}

/// Stores `pending` as the text the window shows once no retired window is still being destroyed or cleaned up; the check and the store share the pending lock with `retire`. Input: the new text. Return: the text it replaced.
fn store(pending: Pending) -> Option<Pending> {
    loop {
        let mut slot = lock(&PENDING);
        if !CLOSING.load(Ordering::Acquire) {
            return slot.replace(pending);
        }
        drop(slot);
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// Destroys the window `ok` accepted through `retire`: the very window that was checked, never a later one built under the same label. Input: the app and the check. Return: the pending text it held, None when `ok` refused.
fn close(app: &AppHandle, ok: impl FnOnce(Option<&Pending>) -> bool) -> Option<Option<Pending>> {
    let (dropped, window) = retire(ok, || app.get_webview_window(WINDOW))?;
    finish(window);
    Some(dropped)
}

/// The page of window `owner` never read its text: close the window, rescue any dictated text, and say so; an older window's watchdog changes nothing. Input: the app and the window's number.
fn stuck_if(app: &AppHandle, owner: u64) {
    let Some(dropped) = close(app, |_| OWNER.load(Ordering::Acquire) == owner) else { return };
    rescue(app, dropped);
    status::report(app, "The preview window did not open and was closed.");
}

/// Stores what the window shows (waiting for a retired window's Destroyed cleanup, which runs on the event loop: callers are the dictation and grab worker threads, never the event loop), builds it if needed and lets it take the keyboard: the text is known, so Ctrl+C can no longer go to it by mistake.
fn publish(app: &AppHandle, pending: Pending) {
    rescue(app, store(pending)); // a dictated text still open is never overwritten silently
    warm(app);
    if app.get_webview_window(WINDOW).is_none() {
        // The window could not be built: the text has nowhere to be shown, so it goes to the clipboard.
        let stranded = lock(&PENDING).take();
        return rescue(app, stranded);
    }
    window::arm(app);
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
    lock(&PENDING).as_ref().filter(|p| p.session == session).map(|p| (p.original.clone(), p.app.clone()))
}

/// Whether the preview window exists.
pub fn is_open(app: &AppHandle) -> bool {
    window::exists(app)
}

/// Focus moved to or from the window: the palette (a list of actions, like a launcher) closes when the user clicks away, unless the window is still in front (ADR 0023). A dictation result stays, since closing it would copy it unasked.
pub fn on_focus(app: &AppHandle, focused: bool) {
    if focused {
        return FOCUS.gained();
    }
    let is_palette = lock(&PENDING).as_ref().is_some_and(|p| p.items.is_some());
    if is_palette && FOCUS.lost(window_guard::in_front(app, WINDOW)) {
        dismiss(app);
    }
}

/// The window was closed by any means (its own buttons, Alt+F4): the text must not linger in memory, and dictated text is copied rather than lost.
pub fn forget(app: &AppHandle) {
    settle(window::exists(app), |dropped| rescue(app, dropped));
}

/// The cleanup of a Destroyed event: unless a newer window exists (then the pending text is its), ends the watchdog, owner and model run, takes the pending text and hands it to `rescue`; only then lets publication go on, so a waiting text is never stored while this cleanup could still take it. Input: whether a window exists now, and what to do with the dropped text.
fn settle(newer_window: bool, rescue: impl FnOnce(Option<Pending>)) {
    if !newer_window {
        window::WATCH.disarm();
        OWNER.store(0, Ordering::Release);
        palette::cancel();
        rescue(lock(&PENDING).take());
    }
    release();
}

/// Closes the window without applying anything (nothing to review after all, or the flow was cancelled). A dictated text still open goes to the clipboard first, as in `forget`: another flow closing the window must never drop what the user said.
pub fn dismiss(app: &AppHandle) {
    rescue(app, close(app, |_| true).flatten());
}

/// What the window shows now, for its page; None when nothing is pending.
#[tauri::command]
pub fn preview_get() -> Option<View> {
    window::WATCH.disarm();
    lock(&PENDING).as_ref().map(|p| View {
        original: p.original.clone(),
        result: p.result.clone(),
        via: p.via.clone(),
        items: p.items.clone(),
        mode: if p.copy { "copy" } else { "paste" },
        session: p.session,
    })
}

/// Whether `p` belongs to `session`: a request of another session (a delayed Apply or close of a closed palette) must not act on it.
fn is_session(p: Option<&Pending>, session: u64) -> bool {
    p.is_some_and(|p| p.session == session)
}

/// Whether Paste of `session` has something to paste (a dictation result, or text from the palette's result box) in `p`; otherwise Paste does nothing and the window stays.
fn applicable(p: Option<&Pending>, has_text: bool, session: u64) -> bool {
    is_session(p, session) && p.is_some_and(|p| p.result.is_some() || (p.items.is_some() && has_text))
}

/// `text` is what the result box holds (the user may have edited it); `id` is the palette row it came from. A saved action whose output is Copy copies instead of pasting.
#[tauri::command]
pub fn preview_apply(app: AppHandle, text: Option<String>, id: Option<String>, session: Option<u64>) {
    if text.as_deref() == Some("") {
        return refuse_empty(&app, session.unwrap_or(0));
    }
    let session = session.unwrap_or(0);
    let Some(Some(p)) = close(&app, |p| applicable(p, text.is_some(), session)) else { return };
    let Some(result) = text.or_else(|| p.result.clone()) else { return };
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

/// An empty result of `session` is never pasted: the window closes with a notice, and dictated text is copied so it is not lost; another session's empty request does nothing.
fn refuse_empty(app: &AppHandle, session: u64) {
    let Some(Some(p)) = close(app, |p| is_session(p, session)) else { return };
    if let Some(text) = p.result.filter(|_| p.keep) {
        copy_dictated(app, &text, "The dictated text was copied to the clipboard. Paste it with Ctrl+V.");
    }
    notice("Error pasting empty text");
}

/// Cancel: also stops the selection flow, so a result that arrives later opens nothing. A close of another session (a page whose text was replaced) does nothing. Input: the app, the session the page shows and the box's text.
#[tauri::command]
pub fn preview_close(app: AppHandle, session: u64, text: Option<String>) {
    let Some((Some(p), window)) = retire(
        |p| is_session(p, session),
        || {
            crate::selection::cancel();
            app.get_webview_window(WINDOW)
        },
    ) else {
        return;
    };
    finish(window);
    // Dictated text is not in any document, so closing the window must not lose it (the box may hold the user's edits).
    if let Some(saved) = p.keep.then(|| text.filter(|t| !t.is_empty()).or(p.result)).flatten() {
        copy_dictated(&app, &saved, "The dictated text was copied to the clipboard. Paste it with Ctrl+V.");
    }
}
