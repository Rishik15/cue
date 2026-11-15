//! File: strip/window.rs
//! Purpose: The strip's webview window: built hidden and unfocusable at the press (so Ctrl+C still goes to the user's app), moved and sized in one step whenever the page asks, shown and given the keyboard the first time, destroyed on close. A watchdog closes a window whose page never asks to be shown, or never finishes its close animation; it ends as soon as the window is shown or closed, so no thread outlives the window.
//! Contents: LABEL — the window label; CLOSE_DEADLINE; exists; warm — build it; watch / unwatch — the watchdog; place — move and size it, and the first time show it with the keyboard; destroy.

use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::Mutex;
use std::time::Duration;

use tauri::{AppHandle, Manager, WebviewWindowBuilder};

use crate::platform::{self, Rect};
use crate::preview::native_handle;
use crate::status;

/// The label of the strip window in `tauri.conf.json`.
pub const LABEL: &str = "strip";
/// The armed watchdog's line: dropping it (`unwatch`, or a newer window's `watch`) wakes the watchdog and ends it.
static WATCH: Mutex<Option<Sender<()>>> = Mutex::new(None);
/// How long a hidden window may wait for its page before it is closed.
const SHOW_DEADLINE: Duration = Duration::from_secs(10);
/// How long a closing window may play its close animation (100 ms on the page) before it is closed anyway.
pub const CLOSE_DEADLINE: Duration = Duration::from_millis(300);

/// Whether the strip window exists. Input: the app. Return: the answer.
pub fn exists(app: &AppHandle) -> bool {
    app.get_webview_window(LABEL).is_some()
}

/// Builds the window hidden, unfocused and unfocusable, as early as the press: the webview is the slow part, so it loads while the selection is copied. Input: the app. Return: whether the window exists afterwards.
pub fn warm(app: &AppHandle) -> bool {
    if exists(app) {
        return true;
    }
    let Some(config) = app.config().app.windows.iter().find(|w| w.label == LABEL) else { return false };
    crate::runtime::ensure();
    match WebviewWindowBuilder::from_config(app, config).and_then(|b| b.focused(false).focusable(false).build()) {
        Ok(_) => {
            watch(app, SHOW_DEADLINE);
            true
        }
        Err(e) => {
            status::report(app, &format!("Could not open the selection strip: {e}"));
            false
        }
    }
}

/// Locks the watchdog's line, surviving a panic elsewhere. Return: the guard.
fn line() -> std::sync::MutexGuard<'static, Option<Sender<()>>> {
    WATCH.lock().unwrap_or_else(|e| e.into_inner())
}

/// One small thread per waiting window: if it is not shown or closed within `deadline` (a page that failed to load, or one that never finishes closing) the strip is closed, so it cannot block every shortcut. Showing or closing the window, or a newer watch, ends the thread at once. Input: the app and the deadline.
pub fn watch(app: &AppHandle, deadline: Duration) {
    let (armed, wait) = mpsc::channel::<()>();
    *line() = Some(armed);
    let app = app.clone();
    let _ = crate::guard::spawn("strip-watch", 64 * 1024, move || {
        if wait.recv_timeout(deadline) == Err(RecvTimeoutError::Timeout) {
            super::dismiss(&app);
        }
    });
}

/// Ends the watchdog, if one waits: the window was shown or is gone.
pub fn unwatch() {
    line().take();
}

/// Moves and sizes the window to `bounds` in one step; the first time (`first`) it is also shown, kept on top and given the keyboard, and the watchdog ends. On the first placement the move is made twice: a move onto a monitor of another scale makes the toolkit resize the window, and the second move puts the exact size back. Input: the app, the physical bounds and whether this is the first placement. Return: false when the window is gone or could not take the keyboard.
pub fn place(app: &AppHandle, bounds: Rect, first: bool) -> bool {
    let Some(window) = app.get_webview_window(LABEL) else { return false };
    let Some(handle) = native_handle(&window) else { return false };
    platform::place_window(handle, bounds);
    if !first {
        return true;
    }
    unwatch();
    platform::place_window(handle, bounds);
    let _ = window.show();
    let _ = window.set_focusable(true);
    platform::float_window(handle);
    platform::focus_window(handle) || window.set_focus().is_ok()
}

/// Destroys the window if it exists. Input: the app.
pub fn destroy(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(LABEL) {
        let _ = window.destroy();
    }
}
