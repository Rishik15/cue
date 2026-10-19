//! Purpose: Makes a window a floating tool window above all normal ones (Windows). The webview toolkit's own always-on-top flag did not hold on a window shown after being built hidden, and window managers that tile or re-stack ordinary windows (GlazeWM and the like) skip tool windows, so the note window sets both styles itself.
//! Contents: float_window — add the tool-window and topmost styles, then put the window at the top of the topmost band without moving, resizing or activating it.

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, HWND_TOPMOST, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOSIZE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
};

pub fn float_window(window: isize) {
    let hwnd = HWND(window as _);
    // SAFETY: plain calls on a window handle; a window that is gone just makes them fail.
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) | (WS_EX_TOOLWINDOW.0 | WS_EX_TOPMOST.0) as isize;
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style);
        let _ = SetWindowPos(hwnd, Some(HWND_TOPMOST), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED);
    }
}
