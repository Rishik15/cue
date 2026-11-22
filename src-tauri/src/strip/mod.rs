//! File: strip/mod.rs
//! Purpose: The selection shortcut's Transform Text strip: a webview window beside the selected text with a few chips, a command list and a result card. Rust owns the copied text, its rows, running a row, delivery and where the window goes; the page owns drawing, keys, filtering and motion. Rust sends the page ONE event (`strip-open`: rows, text, chips, theme and the selection's place) once the page is ready and the text is copied, whichever comes last; the page answers with one command per user action (`commands`).
//! Contents: LAST; OPEN_EVENT; CLOSE_EVENT; Open — the event; warm — build the window at the press; open — hand over the copied text; is_open; close — close after the page's close animation; dismiss — close at once; on_focus — close when the user clicks away; forget — drop the session when the window is gone; send — send the event and log the timing.
//! shortcut: one strip at a time, kept in a process-wide state like the preview window's; enough while the selection shortcut is the only opener.

mod bounds;
mod choose;
#[cfg(test)]
mod choose_tests;
pub mod commands;
mod state;
#[cfg(test)]
mod tests;
mod window;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::palette::{self, Group, Item};
use crate::platform::{self, Rect, Target};
use crate::settings;
use bounds::Dip;
use choose::Candidate;
use state::{Session, State};

/// The settings key holding the id of the last action pasted from the strip; it earns that action a chip next time.
pub const LAST: &str = "strip_last_action";
/// The one event Rust sends the page.
const OPEN_EVENT: &str = "strip-open";
/// The event that asks the page to play its close animation and then call `strip_close`.
const CLOSE_EVENT: &str = "strip-close";

static STATE: Mutex<State> = Mutex::new(State::CLOSED);
/// The window has held the keyboard: clicking away closes it only after that.
static HAD_FOCUS: AtomicBool = AtomicBool::new(false);

/// What the page is sent: everything it needs to paint without asking.
#[derive(Serialize, Clone)]
struct Open<'a> {
    session: u64,
    text: &'a str,
    rows: Vec<state::Row<'a>>,
    chips: &'a [usize],
    theme: settings::Map,
    anchor: Dip,
    work: Dip,
}

/// Locks the state, surviving a panic elsewhere. Return: the guard.
fn state() -> std::sync::MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

/// Builds the strip window hidden at the press, so its page loads while the selection is copied. Input: the app. Return: whether the window exists.
pub fn warm(app: &AppHandle) -> bool {
    if !window::exists(app) {
        *state() = State::CLOSED;
        HAD_FOCUS.store(false, Ordering::Release);
    }
    window::warm(app)
}

/// Whether the strip window exists. Input: the app. Return: the answer.
pub fn is_open(app: &AppHandle) -> bool {
    window::exists(app)
}

/// The copied selection is ready: picks the chips, stores the session and sends it to the page once the page is ready. A window closed meanwhile (the watchdog, a click away) gets nothing and keeps no text. Input: the app, the rows (never empty), the text and its program, the field token, the selection's place and its monitor's work area and scale, and when the shortcut was pressed.
pub fn open(
    app: &AppHandle,
    items: Vec<Item>,
    (text, program): (String, String),
    target: Target,
    (anchor, work, scale): (Rect, Rect, f32),
    pressed: Instant,
) {
    if !window::exists(app) {
        state().session = None;
        return;
    }
    let last = settings::get_str(LAST);
    let candidates: Vec<Candidate> = items.iter().map(|i| Candidate { id: &i.id, saved: i.group == Group::Rule }).collect();
    let chips = choose::chips(&candidates, last.as_deref(), &text);
    let epoch = palette::open();
    let session =
        Session { id: epoch, epoch, items, chips, text, program, target, work, scale, anchor, results: Vec::new(), pressed, shown: false };
    crate::log::note(&format!("strip: rows built at {} ms", pressed.elapsed().as_millis()));
    let mut s = state();
    if s.store(session) {
        send(app, &mut s);
    }
}

/// Sends the open event the caller claimed; a failed send leaves it to be claimed again. Input: the app and the locked state.
fn send(app: &AppHandle, s: &mut State) {
    let Some(session) = s.session.as_ref() else { return };
    let open = Open {
        session: session.id,
        text: &session.text,
        rows: state::rows(&session.items, &session.text),
        chips: &session.chips,
        theme: settings::get_theme(),
        anchor: bounds::to_dip(session.anchor, session.work, session.scale),
        work: bounds::to_dip(session.work, session.work, session.scale),
    };
    let pressed = session.pressed;
    match app.emit_to(window::LABEL, OPEN_EVENT, open) {
        Ok(()) => crate::log::note(&format!("strip: open event sent at {} ms", pressed.elapsed().as_millis())),
        Err(e) => {
            crate::log::note(&format!("strip: the open event failed: {e}"));
            s.unsent();
        }
    }
}

/// Closes the strip the way the user sees it close: nothing is pasted, any model run stops and every later command finds no session, then the page fades the card out and calls `strip_close`. A page that does not answer within `CLOSE_DEADLINE` is closed anyway; a window never shown, or one already closing, closes at once. Input: the app.
pub fn close(app: &AppHandle) {
    let shown = state().session.take().is_some_and(|s| s.shown);
    palette::cancel();
    if !shown || app.emit_to(window::LABEL, CLOSE_EVENT, ()).is_err() {
        return dismiss(app);
    }
    window::watch(app, window::CLOSE_DEADLINE);
}

/// Closes the strip at once without pasting: stops any model run, forgets the text, ends the watchdog and destroys the window. Input: the app.
pub fn dismiss(app: &AppHandle) {
    palette::cancel();
    state().session = None;
    window::unwatch();
    window::destroy(app);
}

/// Focus moved to or from the window: once it has held the keyboard, losing it (a click elsewhere) closes it, unless the window is still in front (taking the keyboard makes WebView2 report a loss and a gain while the window stays in front, so without this the strip closed at once over Edge and Notepad). Input: the app and whether it gained focus.
pub fn on_focus(app: &AppHandle, focused: bool) {
    if focused {
        return HAD_FOCUS.store(true, Ordering::Release);
    }
    let in_front = app.get_webview_window(window::LABEL).and_then(|w| crate::preview::native_handle(&w)).is_some_and(platform::in_front);
    if !in_front && HAD_FOCUS.swap(false, Ordering::AcqRel) {
        close(app);
    }
}

/// The window is gone (closed by any means): its text must not linger, its model run and its watchdog must stop. A destroyed event of an older window while a newer one exists changes nothing. Input: the app.
pub fn forget(app: &AppHandle) {
    if window::exists(app) {
        return;
    }
    palette::cancel();
    window::unwatch();
    *state() = State::CLOSED;
}
