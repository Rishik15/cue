//! Purpose: Puts a window above all normal ones (Windows). The webview toolkit's own always-on-top flag did not hold on a window shown after being built hidden, so the note window asks the system directly.
//! Contents: float_window — put the window at the top of the topmost band without moving, resizing or activating it.

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE};

pub fn float_window(window: isize) {
    // SAFETY: a plain call on a window handle; a window that is gone just makes it fail.
    let _ = unsafe { SetWindowPos(HWND(window as _), Some(HWND_TOPMOST), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE) };
}
