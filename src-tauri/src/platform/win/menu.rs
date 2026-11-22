//! File: menu.rs
//! Purpose: Leave the menu mode an app's menu bar enters when the Alt of Cue's shortcut is released. Windows 11 Notepad (and classic menu bars) activate the menu on every Alt release, even after a chord with another key, and no injected mask key stops it; UI Automation then reports the menu item (File) as the focused element, so the selection cannot be read and a result cannot be delivered. One Escape ends the mode and the document has the focus again.
//! Contents: in_menu_bar — whether a field key names a menu-bar item; leave_for — leave the menu mode when it is not where the token's field is.

use std::time::{Duration, Instant};

use windows::Win32::UI::Input::KeyboardAndMouse::VK_ESCAPE;

use super::context::focused_field;
use super::focus::foreground;
use super::input::Lift;
use super::insert::{send, taps};
use crate::platform::Target;

/// How long the app gets to move the focus out of its menu bar after the Escape.
const LEAVE_WAIT: Duration = Duration::from_millis(500);

/// Whether a field key (control type and automation id of the element and its ancestors) names a menu item inside a menu bar: UI Automation control type 50011 is a menu item, 50010 a menu bar. Input: the key. Return: the answer.
fn in_menu_bar(key: &str) -> bool {
    key.starts_with("50011:") && key.split('/').skip(1).any(|segment| segment.starts_with("50010:"))
}

/// When the focused element is a menu-bar item of the window `want` names and `want` itself is not (the user did not open the menu, the shortcut's Alt did), presses Escape and waits until the focus has left the menu bar. Does nothing for any other state or window. Input: the token of the place the keys or the read are for. Call from a worker thread: it waits for another process.
pub fn leave_for(want: &Target) {
    if want.window == 0 || in_menu_bar(&want.key) || foreground().window != want.window {
        return;
    }
    if !focused_field().is_some_and(|field| in_menu_bar(&field.key)) {
        return;
    }
    crate::log::note("menu: the shortcut's Alt activated the menu bar, leaving it");
    let sent = Lift::new().and_then(|_lift| send(&taps(VK_ESCAPE, 1)));
    if let Err(why) = sent {
        return crate::log::note(&format!("menu: Escape was not sent: {why}"));
    }
    let start = Instant::now();
    while start.elapsed() < LEAVE_WAIT && focused_field().is_some_and(|field| in_menu_bar(&field.key)) {
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(test)]
mod tests {
    use super::in_menu_bar;

    /// A menu-bar item is told by its own type and a menu-bar ancestor; the document, a menu item with no menu bar above it and a menu bar itself are not.
    #[test]
    fn only_an_item_inside_a_menu_bar_is_the_menu_mode() {
        assert!(in_menu_bar("50011:File/50010:MenuBar/50033:/50032:"));
        assert!(!in_menu_bar("50030:/50033:/50032:"));
        assert!(!in_menu_bar("50011:Copy/50033:/50032:"));
        assert!(!in_menu_bar("50010:MenuBar/50033:"));
        assert!(!in_menu_bar(""));
    }
}
