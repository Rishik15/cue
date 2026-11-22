//! File: dictionary/window.rs
//! Purpose: The dictionary's webview window: built hidden and unfocusable at the press (so Ctrl+C still goes to the user's app and a word's card never steals the keyboard), moved and sized in one step when the page asks, shown the first time (the search window also takes the keyboard), watched for the user leaving while a card is up, destroyed on close. A watchdog closes a window whose page never asks to be shown; it ends as soon as the window is shown or gone, so no thread outlives the window.
//! Contents: MARGIN — the transparent border the page draws its shadow in; exists; warm — build it; arm / unwatch — the watchdog; show — move, size and (first time) show it; closing_on_err — close the lookup when a step failed; book / kept / release / release_all — the pack connections kept while the window is open (`release_all` when a pack changes); follow — start the click-away watch of a card; destroy.
//! Every window belongs to one lookup and is named by its number, so work of an older lookup (a late placement, a watchdog, a close) can only find its own window, never a newer one built under the same name.

use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::{AppHandle, Manager, WebviewWindowBuilder};

use super::book::Book;
use super::label::label;
use super::state::arm_slot;
use crate::platform::{self, Rect};
use crate::preview::native_handle;
use crate::status;

/// The label of the window in `tauri.conf.json`; a built window's label is made by `label` from the lookup's number.
const BASE: &str = "dictionary";
/// The transparent border round the card, in dip, so its shadow and arrow have room; the page and the placement agree on it.
pub const MARGIN: f64 = 16.0;
/// How long a hidden window may wait for its page before it is closed.
const SHOW_DEADLINE: Duration = Duration::from_secs(10);
/// The armed watchdog's lookup and line: dropping the line wakes the watchdog and ends it.
static WATCH: Mutex<Option<(u64, Sender<()>)>> = Mutex::new(None);
/// The lookup and click-away watch of the card that is up; dropping the watch removes its hooks.
static FRONT: Mutex<Option<(u64, platform::FrontWatch)>> = Mutex::new(None);

/// The book whose pack connections window `id` keeps open while it is up (the page searches on every keystroke); dropped with the window, so a closed window holds no file and idle costs nothing.
static BOOK: Mutex<Option<(u64, Arc<Book>)>> = Mutex::new(None);

/// Builds the window of lookup `id` hidden, unfocused and unfocusable at the press: the webview is the slow part, so it loads while the selection is read. Input: the app and the lookup's number. Return: whether the window exists afterwards.
pub fn warm(app: &AppHandle, id: u64) -> bool {
    if app.get_webview_window(&label(id)).is_some() {
        return true;
    }
    let Some(config) = app.config().app.windows.iter().find(|w| w.label == BASE) else {
        crate::log::note("dictionary: no dictionary window in the config");
        return false;
    };
    let config = tauri::utils::config::WindowConfig { label: label(id), ..config.clone() };
    crate::runtime::ensure();
    match WebviewWindowBuilder::from_config(app, &config).and_then(|b| b.focused(false).focusable(false).build()) {
        Ok(_) => {
            arm(app, id);
            true
        }
        Err(e) => {
            status::report(app, &format!("Could not open the dictionary: {e}"));
            false
        }
    }
}

/// Locks the watchdog's line, surviving a panic elsewhere.
fn line() -> std::sync::MutexGuard<'static, Option<(u64, Sender<()>)>> {
    WATCH.lock().unwrap_or_else(|e| e.into_inner())
}

/// Arms the watchdog of window `id` only while lookup `id` is still the wanted one (`arm_slot`: the check and the registration under the lifecycle lock). One small thread per waiting window: if window `id` is not shown or closed within `SHOW_DEADLINE` (a page that failed to load) its lookup is closed, so it cannot block the shortcut. Showing or closing the window ends the thread at once. Input: the app and the window's number.
fn arm(app: &AppHandle, id: u64) {
    let (armed, wait) = mpsc::channel::<()>();
    if !arm_slot(&super::state(), &mut line(), id, || armed) {
        return;
    }
    let app = app.clone();
    let closer = app.clone();
    let spawned = crate::guard::spawn("dictionary-watch", 64 * 1024, move || {
        if wait.recv_timeout(SHOW_DEADLINE) == Err(RecvTimeoutError::Timeout) {
            let closed = super::dismiss_if(&app, id, |_| true);
            crate::log::note(&format!("dictionary: watchdog expired, the page never asked to be shown, closed {closed}"));
        }
    });
    closing_on_err(spawned, "start the watchdog", || {
        super::dismiss_if(&closer, id, |_| true);
    });
}

/// Passes a step's outcome on, closing the lookup when the step failed so a window nobody can see or time out never blocks the next shortcut. Input: the outcome, what was tried (for the log, no user text) and what closes the lookup. Return: whether the step worked.
pub(super) fn closing_on_err<T, E: std::fmt::Display>(outcome: Result<T, E>, what: &str, close: impl FnOnce()) -> bool {
    let Err(e) = outcome else { return true };
    crate::log::note(&format!("dictionary: could not {what}: {e}"));
    close();
    false
}

/// Ends the watchdog of window `id`, if it waits (a newer window's watchdog stays).
fn unwatch(id: u64) {
    let mut armed = line();
    if armed.as_ref().is_some_and(|(owner, _)| *owner == id) {
        armed.take();
    }
}

/// Moves and sizes window `id` to `bounds` in one step; the first time (`first`) it is also shown and kept on top, and, when it is a search window (`focus`), given the keyboard; the watchdog ends. On the first placement the move is made twice: a move onto a monitor of another scale makes the toolkit resize the window, and the second move puts the exact size back. Input: the app, the window's number, the physical bounds, whether this is the first placement and whether the window takes the keyboard. Return: false when the window is gone, could not be shown (its lookup is closed) or could not take the keyboard.
pub fn show(app: &AppHandle, id: u64, bounds: Rect, first: bool, focus: bool) -> bool {
    let Some(window) = app.get_webview_window(&label(id)) else { return false };
    let Some(handle) = native_handle(&window) else { return false };
    platform::place_window(handle, bounds);
    if !first {
        return true;
    }
    unwatch(id);
    platform::place_window(handle, bounds);
    if !closing_on_err(window.show(), "show the window", || {
        super::dismiss_if(app, id, |_| true);
    }) {
        return false;
    }
    if focus {
        let _ = window.set_focusable(true);
    }
    platform::float_window(handle);
    if !focus {
        return true;
    }
    let focused = platform::focus_window(handle) || window.set_focus().is_ok();
    crate::log::note(&format!("dictionary: search window took the keyboard {focused}"));
    focused
}

/// The book window `id` searches, opened on its first search and kept until the window is destroyed. Input: the app and the window's number. Return: the book.
pub fn book(app: &AppHandle, id: u64) -> Arc<Book> {
    kept(id, || super::installed_book(app))
}

/// `book` with the maker given (the tests keep a book of their own in the real slot). Input: the window's number and the maker. Return: the book.
pub(super) fn kept(id: u64, make: impl FnOnce() -> Book) -> Arc<Book> {
    cached(&mut BOOK.lock().unwrap_or_else(|e| e.into_inner()), id, make)
}

/// The value kept for window `id`: the kept one when it is `id`'s, a new one from `make` kept in its place, and a new one not kept when the kept one is a newer window's (numbers only grow, so a late search of a closed window never evicts the open window's). Input: the slot, the window's number and the maker. Return: the value.
pub(super) fn cached<T>(kept: &mut Option<(u64, Arc<T>)>, id: u64, make: impl FnOnce() -> T) -> Arc<T> {
    match kept.as_ref() {
        Some((owner, value)) if *owner == id => value.clone(),
        Some((owner, _)) if *owner > id => Arc::new(make()),
        _ => {
            let value = Arc::new(make());
            *kept = Some((id, value.clone()));
            value
        }
    }
}

/// Drops the pack connections window `id` kept (a newer window's stay). Input: the number.
pub fn release(id: u64) {
    drop_kept(&mut BOOK.lock().unwrap_or_else(|e| e.into_inner()), id);
}

/// Empties `kept` when it is window `id`'s. Input: the slot and the window's number.
pub(super) fn drop_kept<T>(kept: &mut Option<(u64, Arc<T>)>, id: u64) {
    if kept.as_ref().is_some_and(|(owner, _)| *owner == id) {
        kept.take();
    }
}

/// Drops the pack connections of whichever window kept them, so a pack can be replaced or removed (`catalog::changing` calls it with reads held off; the open Search window opens the new pack on its next search).
pub(super) fn release_all() {
    BOOK.lock().unwrap_or_else(|e| e.into_inner()).take();
}

/// Starts the watch that closes card `id` when the user clicks or switches away from it; a watch of an older card is ended. Input: the card's number, its bounds, the window the word is in and what to call (with the number) when the user has left. Return: false when the hooks could not be installed, so the card must not be shown.
pub fn follow(id: u64, bounds: Rect, source: isize, on_leave: fn(u64)) -> bool {
    let watch = platform::watch_front(id, bounds, source, on_leave);
    let started = watch.is_some();
    *FRONT.lock().unwrap_or_else(|e| e.into_inner()) = watch.map(|w| (id, w));
    started
}

/// Destroys window `id` if it exists, ends its watches and drops its pack connections (an older or newer window's stay). Input: the app and the number.
pub fn destroy(app: &AppHandle, id: u64) {
    unwatch(id);
    release(id);
    {
        let mut front = FRONT.lock().unwrap_or_else(|e| e.into_inner());
        if front.as_ref().is_some_and(|(owner, _)| *owner == id) {
            front.take();
        }
    }
    if let Some(window) = app.get_webview_window(&label(id)) {
        let _ = window.destroy();
    }
}
