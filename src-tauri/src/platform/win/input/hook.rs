//! File: win/input/hook.rs
//! Purpose: The low-level keyboard hook callback. It does the minimum: classify the event (Cue's own mark, the AltGr phantom), look the key up, hand the edge to the reducer and return pass or swallow. No lock that can block, no IO, no UI, no feature call and no SendInput; a panic fails open and resets the key state.
//! Contents: install / uninstall — the hook handle; keyboard_proc — the callback.

use std::time::Instant;

use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, SetWindowsHookExW, UnhookWindowsHookEx, HHOOK, KBDLLHOOKSTRUCT, LLKHF_EXTENDED, WH_KEYBOARD_LL, WM_KEYDOWN,
    WM_SYSKEYDOWN,
};

use super::inject::MARK;
use super::pump::{request_sync, with};
use crate::input::{is_phantom_ctrl, slot, Edge, Origin, Verdict};

/// Installs the keyboard hook on this thread, or the reason it could not be.
pub(super) fn install() -> Result<HHOOK, String> {
    unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), None, 0) }
        .map_err(|e| format!("Windows refused the keyboard hook: {e}"))
}

/// Removes a hook. Safety: the handle came from `install` and is removed once.
pub(super) fn uninstall(hook: HHOOK) {
    unsafe {
        let _ = UnhookWindowsHookEx(hook);
    }
}

/// Hook entry: contained, so a panic never unwinds into Windows; a panic passes the key and resets the key state.
unsafe extern "system" fn keyboard_proc(code: i32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if code < 0 {
        return CallNextHookEx(None, code, wp, lp);
    }
    match crate::guard::contain(|| on_key(wp, lp)) {
        Some(true) => LRESULT(1),
        Some(false) => CallNextHookEx(None, code, wp, lp),
        None => {
            let _ = with(|st| st.rt.fault(Instant::now()));
            CallNextHookEx(None, code, wp, lp)
        }
    }
}

/// Runs one event through the reducer. `lp` must point at the `KBDLLHOOKSTRUCT` Windows passes when `code` is not negative. Returns whether to swallow it.
unsafe fn on_key(wp: WPARAM, lp: LPARAM) -> bool {
    let event = &*(lp.0 as *const KBDLLHOOKSTRUCT);
    let (vk, scan) = (event.vkCode as u16, event.scanCode);
    let extended = event.flags.0 & LLKHF_EXTENDED.0 != 0;
    let down = matches!(wp.0 as u32, WM_KEYDOWN | WM_SYSKEYDOWN);
    let origin = if event.dwExtraInfo == MARK {
        Origin::Cue
    } else if is_phantom_ctrl(vk, scan) {
        Origin::Phantom
    } else {
        Origin::Physical
    };
    let Some(slot) = slot(vk, scan, extended) else { return false };
    let swallow = with(|st| st.rt.edge(Edge { slot, down, origin }, Instant::now()) == Verdict::Swallow).unwrap_or(false);
    request_sync();
    swallow
}
