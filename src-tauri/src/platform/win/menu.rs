//! File: menu.rs
//! Purpose: Leave the menu mode an app's menu bar enters when the Alt of Cue's shortcut is released. Windows 11 Notepad (and classic menu bars) activate the menu on every Alt release, even after a chord with another key, and no injected mask key stops it; UI Automation then reports the menu item (File) as the focused element, so the selection cannot be read and a result cannot be delivered. One Escape ends the mode and the document has the focus again.
//! Only a menu the shortcut's Alt started is left: the token taken at the press says whether a menu bar already had the focus (`Target::menu_open`) and when it was taken (`Target::pressed`), and recovery stops after `RECENT`, when the user may have opened a menu themselves. The decision is the pure `should_escape`.
//! Contents: is_bar_item — whether a field key names a top-level menu-bar item; should_escape — the decision; escapable — the decision on the live state; leave_held — leave the menu mode with the transaction gate already held; leave_for — the same for a caller that does not hold it.

use std::time::{Duration, Instant};

use windows::Win32::UI::Input::KeyboardAndMouse::VK_ESCAPE;

use super::clipboard_tx::{self, Tx};
use super::context::focused_field;
use super::focus::foreground;
use super::input::Lift;
use super::insert::{send, taps};
use crate::platform::Target;

/// How long after the press a menu is still taken for the shortcut's Alt: the Alt is released within a moment, and after this long the user may have opened a menu themselves.
const RECENT: Duration = Duration::from_secs(2);
/// How long the app gets to move the focus out of its menu bar after the Escape.
const LEAVE_WAIT: Duration = Duration::from_millis(500);

/// Whether a field key (control type and automation id of the element and its ancestors) names a top-level menu-bar item: UI Automation control type 50011 is a menu item and its parent is the menu bar (50010). An item of an expanded submenu has a menu between, so it is not one. Input: the key. Return: the answer.
pub(super) fn is_bar_item(key: &str) -> bool {
    let mut segments = key.split('/');
    segments.next().is_some_and(|own| own.starts_with("50011:")) && segments.next().is_some_and(|parent| parent.starts_with("50010:"))
}

/// Whether the window and time still allow recovery: the token records a press no longer ago than `RECENT`, no menu was open at the press, and the foreground is the token's window, process and thread. Input: the press token, the foreground now and the time since the press (None when the token has none). Return: the answer.
fn window_allows(press: &Target, front: &Target, elapsed: Option<Duration>) -> bool {
    press.window != 0
        && !press.menu_open
        && elapsed.is_some_and(|e| e <= RECENT)
        && front.window == press.window
        && (press.pid == 0 || front.pid == press.pid)
        && (press.thread == 0 || front.thread == press.thread)
}

/// Whether Escape should be sent: `window_allows` and the focused element is a top-level menu-bar item. Input: the press token, the foreground now, the focused element's key and the time since the press. Return: the answer.
pub(super) fn should_escape(press: &Target, front: &Target, focused_key: &str, elapsed: Option<Duration>) -> bool {
    window_allows(press, front, elapsed) && is_bar_item(focused_key)
}

/// `should_escape` on the live state; the focused element is read only when the window and time allow. Input: the press token. Return: the answer.
fn escapable(press: &Target) -> bool {
    let (front, elapsed) = (foreground(), press.pressed.map(|at| at.elapsed()));
    window_allows(press, &front, elapsed) && focused_field().is_some_and(|field| should_escape(press, &front, &field.key, elapsed))
}

/// Ends the menu mode the shortcut's Alt started, for a caller that already holds the transaction gate (`_tx` proves it; the gate is not entered again). The state is checked again after the held keys are lifted and right before the Escape goes out, so a window or popup that appeared meanwhile gets nothing. Input: the press token and the held gate. Call from a worker thread: it waits for another process.
pub fn leave_held(press: &Target, _tx: &Tx) {
    if !escapable(press) {
        return;
    }
    let lift = match Lift::new() {
        Ok(lift) => lift,
        Err(why) => return crate::log::note(&format!("menu: the held keys could not be lifted: {why}")),
    };
    if !escapable(press) {
        return crate::log::note("menu: the state changed before the Escape, nothing was sent");
    }
    crate::log::note("menu: the shortcut's Alt activated the menu bar, leaving it");
    if let Err(why) = send(&taps(VK_ESCAPE, 1)) {
        return crate::log::note(&format!("menu: Escape was not sent: {why}"));
    }
    drop(lift);
    let start = Instant::now();
    while start.elapsed() < LEAVE_WAIT && focused_field().is_some_and(|field| is_bar_item(&field.key)) {
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// `leave_held` for a caller outside the transaction gate: the gate is entered only when recovery looks needed, and a busy gate skips it. Input: the press token.
pub fn leave_for(press: &Target) {
    if !escapable(press) {
        return;
    }
    if let Ok(tx) = clipboard_tx::enter() {
        leave_held(press, &tx);
    }
}

#[cfg(test)]
mod tests {
    use super::{is_bar_item, should_escape, RECENT};
    use crate::platform::Target;
    use std::time::Duration;

    const BAR: &str = "50011:File/50010:MenuBar/50033:/50032:";

    /// The token of a press in window 7 with no menu open.
    fn press() -> Target {
        Target { window: 7, pid: 1, thread: 2, pressed: None, ..Target::default() }
    }

    /// Only a top-level item of a menu bar is the menu mode: the document, an item with no bar, a bar itself and an item of an expanded submenu (a menu between) are not.
    #[test]
    fn only_a_top_level_bar_item_is_the_menu_mode() {
        assert!(is_bar_item(BAR));
        assert!(!is_bar_item("50030:/50033:/50032:"));
        assert!(!is_bar_item("50011:Copy/50033:/50032:"));
        assert!(!is_bar_item("50010:MenuBar/50033:"));
        assert!(!is_bar_item("50011:Save/50009:File/50010:MenuBar/50032:"));
        assert!(!is_bar_item(""));
    }

    /// The shortcut's Alt left a menu bar active soon after the press in the same window: Escape is right; the same state with the document focused needs nothing.
    #[test]
    fn a_menu_soon_after_the_press_in_the_same_window_is_left() {
        let ms = Some(Duration::from_millis(300));
        assert!(should_escape(&press(), &press(), BAR, ms));
        assert!(should_escape(&press(), &press(), BAR, Some(RECENT)));
        assert!(!should_escape(&press(), &press(), "50030:", ms));
    }

    /// A menu the user already had open at the press, or one met after the time allowed, is theirs: nothing is sent.
    #[test]
    fn a_deliberate_or_late_menu_is_left_alone() {
        let ms = Some(Duration::from_millis(300));
        assert!(!should_escape(&Target { menu_open: true, ..press() }, &press(), BAR, ms));
        assert!(!should_escape(&press(), &press(), BAR, Some(RECENT + Duration::from_millis(1))));
        assert!(!should_escape(&press(), &press(), BAR, None));
    }

    /// Another window, process or thread in front, or a token for no window, gets no Escape.
    #[test]
    fn another_window_gets_no_escape() {
        let ms = Some(Duration::from_millis(300));
        assert!(!should_escape(&press(), &Target { window: 8, ..press() }, BAR, ms));
        assert!(!should_escape(&press(), &Target { pid: 9, ..press() }, BAR, ms));
        assert!(!should_escape(&press(), &Target { thread: 9, ..press() }, BAR, ms));
        assert!(!should_escape(&Target::default(), &press(), BAR, ms));
    }
}
