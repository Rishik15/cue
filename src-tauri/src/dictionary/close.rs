//! File: dictionary/close.rs
//! Purpose: Every way the dictionary window ends: a close by the shortcut, Escape or typing, a click away from the card, the search window losing the keyboard, and the window being gone. Each close names the lookup it is for, so an older one cannot close a newer one.
//! Contents: dismiss / dismiss_if — close what is up (now, or when the number and a condition on the state allow); left — the user left the card; on_focus — the window gained or lost the keyboard; forget — the window is gone.

use tauri::{AppHandle, Manager};

use super::state::State;
use super::{label, since_press, state, window, APP};

/// Closes whatever is up at once: the lookup is dropped, the session forgotten and the window destroyed. Input: the app.
pub fn dismiss(app: &AppHandle) {
    let id = state().close();
    window::destroy(app, id);
}

/// Closes lookup or window `id` when it is still the current one and `allowed` says so, in one step, and destroys its window; an older number changes nothing. Input: the app, the number and the condition (on the locked state). Return: whether it was closed.
pub(super) fn dismiss_if(app: &AppHandle, id: u64, allowed: impl FnOnce(&State) -> bool) -> bool {
    let closed = state().close_if(id, allowed);
    if let Some(id) = closed {
        window::destroy(app, id);
    }
    closed.is_some()
}

/// The user clicked or switched away from card `id` (called on its click-away watch's thread, which names it): closes it when it is still the open card.
pub(super) fn left(id: u64) {
    if let Some(app) = APP.get() {
        if dismiss_if(app, id, State::is_card) {
            crate::log::note(&format!("dictionary: the card was closed by the user leaving it, {} ms after the press", since_press()));
        }
    }
}

/// Focus moved to or from window `label`: once the search window has held the keyboard, losing it (a click elsewhere) closes it, unless the window is still in front (`State::leaves_on_blur`: taking the keyboard makes WebView2 report a loss and a gain while the window stays in front). A card never takes the keyboard, so it never gets here. Each change is logged with whether the window had held the keyboard and is in front. Input: the app, the window's label and whether it gained focus.
pub fn on_focus(app: &AppHandle, label: &str, focused: bool) {
    let Some(id) = label::id_of(label) else { return };
    if focused {
        state().focused(id);
        return crate::log::note(&format!("dictionary: focus gained at {} ms", since_press()));
    }
    let handle = app.get_webview_window(label).and_then(|w| crate::preview::native_handle(&w));
    let in_front = handle.is_some_and(crate::platform::in_front);
    let mut had_focus = false;
    let closed = dismiss_if(app, id, |s| {
        had_focus = s.held_focus(id);
        s.leaves_on_blur(id, in_front)
    });
    crate::log::note(&format!(
        "dictionary: focus lost at {} ms, had_focus {had_focus}, in front {in_front}, closed {closed}",
        since_press()
    ));
}

/// The window `label` is gone (closed by any means): when it was the current one nothing may stay marked as up, and the next press starts a lookup again. A destroyed event of an older window changes nothing. Input: the app and the label.
pub fn forget(app: &AppHandle, label: &str) {
    if let Some(id) = label::id_of(label) {
        if dismiss_if(app, id, |_| true) {
            crate::log::note("dictionary: the current window was destroyed, the lookup is closed");
        }
    }
}
