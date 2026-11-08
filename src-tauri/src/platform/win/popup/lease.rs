//! File: lease.rs
//! Purpose: Ties the open tray menu to the input module. The menu never takes focus, so the input module's popup lease supplies what activation would have: the menu keys (Escape, Enter, Up, Down) and a dismiss on an outside click or a window-switching key. The lease's inbox posts those to the menu window; a window that is gone makes the post fail, which revokes the lease.
//! Contents: KEY_MESSAGE — posted to the menu window for a menu key, `wparam` the virtual key; acquire — opens the lease for a menu window.

use windows::Win32::Foundation::{HWND, LPARAM, RECT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_DOWN, VK_ESCAPE, VK_RETURN, VK_UP};
use windows::Win32::UI::WindowsAndMessaging::{GetWindowRect, PostMessageW, WM_APP, WM_CLOSE};

use crate::input::{PopupKey, PopupLease, Rect};

pub const KEY_MESSAGE: u32 = WM_APP + 1;

/// Opens the lease for `menu`, or says why not. The inbox holds only the window handle as a number, so it can be called from the input thread. Safety: reads the rectangle of a window this thread just made.
pub fn acquire(menu: HWND, open: crate::platform::PopupOpen) -> Result<PopupLease, String> {
    let mut rect = RECT::default();
    unsafe { GetWindowRect(menu, &mut rect) }.map_err(|e| e.to_string())?;
    let handle = menu.0 as isize;
    let inbox = move |key: PopupKey| {
        let hwnd = HWND(handle as *mut _);
        let (msg, vk) = match key {
            PopupKey::Escape => (KEY_MESSAGE, VK_ESCAPE.0),
            PopupKey::Enter => (KEY_MESSAGE, VK_RETURN.0),
            PopupKey::Up => (KEY_MESSAGE, VK_UP.0),
            PopupKey::Down => (KEY_MESSAGE, VK_DOWN.0),
            PopupKey::Dismiss => (WM_CLOSE, 0),
        };
        unsafe { PostMessageW(Some(hwnd), msg, WPARAM(vk as usize), LPARAM(0)) }.is_ok()
    };
    open(Rect { left: rect.left, top: rect.top, right: rect.right, bottom: rect.bottom }, Box::new(inbox))
}
