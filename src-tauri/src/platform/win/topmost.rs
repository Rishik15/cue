//! File: topmost.rs
//! Purpose: Puts a window above all normal ones (Windows). The webview toolkit's own always-on-top flag did not hold on a window shown after being built hidden, so the note window asks the system directly.
//! Contents: float_window — put the window at the top of the topmost band without moving, resizing or activating it; place_window — move and size it in one step, in that band, without activating it.

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE};

use crate::platform::Rect;

/// Puts `window` at the top of the topmost band without moving, resizing or activating it. Input: a window handle. Safety: a stale handle only makes the call fail. Safety: A plain call on a window handle; a window that is gone just makes it fail.
pub fn float_window(window: isize) {
    let _ = unsafe { SetWindowPos(HWND(window as _), Some(HWND_TOPMOST), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE) };
}

/// Moves and sizes `window` to `bounds` (physical pixels) in one step, so a window that grows upwards never shows a frame at the old place with the new size; it stays in the topmost band and is not activated. Input: a window handle and its new bounds. Safety: A plain call on a window handle; a window that is gone just makes it fail.
pub fn place_window(window: isize, bounds: Rect) {
    let (w, h) = (bounds.right - bounds.left, bounds.bottom - bounds.top);
    let _ = unsafe { SetWindowPos(HWND(window as _), Some(HWND_TOPMOST), bounds.left, bounds.top, w, h, SWP_NOACTIVATE) };
}
