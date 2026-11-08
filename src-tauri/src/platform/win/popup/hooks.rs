//! File: hooks.rs
//! Purpose: Let the menu behave like Pouchy's popup without ever taking focus. A menu that activates itself makes Windows
//! close the tray overflow panel it was opened from, so the menu window is non-activating and these two low-level hooks,
//! installed only while it is open, supply what activation would have: dismiss on an outside click or a window-switching key, and keyboard control.
//! Contents: Hooks — installs both hooks and removes them on drop; mouse_proc / key_proc — the hook procedures, each under `guard::contain` so a panic never unwinds into Windows.

use std::cell::Cell;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::PtInRect;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    VK_DOWN, VK_ESCAPE, VK_LMENU, VK_LWIN, VK_MENU, VK_RETURN, VK_RMENU, VK_RWIN, VK_TAB, VK_UP,
};
use windows::Win32::UI::WindowsAndMessaging::*;

/// Posted to the menu window for Esc, Up, Down and Enter; `wparam` is the virtual key.
pub const KEY_MESSAGE: u32 = WM_APP + 1;

/// Keys that switch to another window (Alt, Tab, Windows): the menu closes on them and the key goes on to the system, so the menu never stays above the app the user switched to or keeps its arrow keys.
const SWITCH_KEYS: [u16; 6] = [VK_MENU.0, VK_LMENU.0, VK_RMENU.0, VK_TAB.0, VK_LWIN.0, VK_RWIN.0];

thread_local! { static MENU: Cell<isize> = const { Cell::new(0) }; }

/// The menu window of this thread's open menu.
fn menu_window() -> HWND {
    HWND(MENU.get() as *mut _)
}

/// Hook entry for mouse input: contained, and passed on when the body panics.
unsafe extern "system" fn mouse_proc(code: i32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    crate::guard::contain(|| on_mouse(code, wp, lp)).unwrap_or_else(|| CallNextHookEx(None, code, wp, lp))
}

/// Closes the menu on a click outside it. `lp` must point at the `MSLLHOOKSTRUCT` Windows passes when `code` is not negative. The click itself still goes through.
unsafe fn on_mouse(code: i32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let pressed = matches!(wp.0 as u32, WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_XBUTTONDOWN);
    if code >= 0 && pressed {
        let point = (*(lp.0 as *const MSLLHOOKSTRUCT)).pt;
        let mut rect = RECT::default();
        let inside = GetWindowRect(menu_window(), &mut rect).is_ok() && PtInRect(&rect, point).as_bool();
        if !inside {
            let _ = PostMessageW(Some(menu_window()), WM_CLOSE, WPARAM(0), LPARAM(0));
        }
    }
    CallNextHookEx(None, code, wp, lp)
}

/// Hook entry for keyboard input: contained, and passed on when the body panics.
unsafe extern "system" fn key_proc(code: i32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    crate::guard::contain(|| on_key(code, wp, lp)).unwrap_or_else(|| CallNextHookEx(None, code, wp, lp))
}

/// Menu keys and window-switching keys. `lp` must point at the `KBDLLHOOKSTRUCT` Windows passes when `code` is not negative. Swallowed: the focused app must not also see them. Keys Cue injects itself (it lifts and restores held modifiers) are never the user's menu keys.
unsafe fn on_key(code: i32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if code >= 0 && matches!(wp.0 as u32, WM_KEYDOWN | WM_SYSKEYDOWN) {
        let event = &*(lp.0 as *const KBDLLHOOKSTRUCT);
        let key = event.vkCode as u16;
        if event.dwExtraInfo != super::super::input::MARK && [VK_ESCAPE.0, VK_UP.0, VK_DOWN.0, VK_RETURN.0].contains(&key) {
            let _ = PostMessageW(Some(menu_window()), KEY_MESSAGE, WPARAM(key as usize), LPARAM(0));
            return LRESULT(1);
        }
        if event.dwExtraInfo != super::super::input::MARK && SWITCH_KEYS.contains(&key) {
            let _ = PostMessageW(Some(menu_window()), WM_CLOSE, WPARAM(0), LPARAM(0));
        }
    }
    CallNextHookEx(None, code, wp, lp)
}

/// The mouse and keyboard hooks of one open menu; dropping it removes them.
pub struct Hooks(HHOOK, HHOOK);

impl Hooks {
    /// Both hooks run on this thread's message loop, which `show_menu` pumps while the menu is open.
    pub fn install(menu: HWND) -> Option<Hooks> {
        MENU.set(menu.0 as isize);
        unsafe {
            let mouse = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), None, 0).ok()?;
            let Ok(key) = SetWindowsHookExW(WH_KEYBOARD_LL, Some(key_proc), None, 0) else {
                let _ = UnhookWindowsHookEx(mouse);
                return None;
            };
            Some(Hooks(mouse, key))
        }
    }
}

impl Drop for Hooks {
    /// Removes both hooks and forgets the menu window. Safety: both handles came from `install` and are removed once.
    fn drop(&mut self) {
        unsafe {
            let _ = UnhookWindowsHookEx(self.0);
            let _ = UnhookWindowsHookEx(self.1);
        }
        MENU.set(0);
    }
}
