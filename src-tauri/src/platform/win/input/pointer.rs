//! File: win/input/pointer.rs
//! Purpose: The button-only mouse hook. It exists only while a surface (an open tray popup, a recording with registered regions) needs outside clicks, so an idle Cue has no mouse hook. It never swallows a click.
//! Contents: install / uninstall — the hook handle; mouse_proc — the callback.

use std::time::Instant;

use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, SetWindowsHookExW, UnhookWindowsHookEx, HHOOK, MSLLHOOKSTRUCT, WH_MOUSE_LL, WM_LBUTTONDOWN, WM_MBUTTONDOWN,
    WM_RBUTTONDOWN, WM_XBUTTONDOWN,
};

use super::pump::{request_sync, with};

/// Installs the mouse hook on this thread, or the reason it could not be.
pub(super) fn install() -> Result<HHOOK, String> {
    unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), None, 0) }.map_err(|e| format!("Windows refused the mouse hook: {e}"))
}

/// Removes a hook. Safety: the handle came from `install` and is removed once.
pub(super) fn uninstall(hook: HHOOK) {
    unsafe {
        let _ = UnhookWindowsHookEx(hook);
    }
}

/// Hook entry: contained, and the click always goes on.
unsafe extern "system" fn mouse_proc(code: i32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if code >= 0 && matches!(wp.0 as u32, WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_XBUTTONDOWN) {
        let point = (*(lp.0 as *const MSLLHOOKSTRUCT)).pt;
        let _ = crate::guard::contain(|| {
            let _ = with(|st| st.rt.pointer_down(point.x, point.y, Instant::now()));
            request_sync();
        });
    }
    CallNextHookEx(None, code, wp, lp)
}
