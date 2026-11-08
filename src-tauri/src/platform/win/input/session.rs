//! File: win/input/session.rs
//! Purpose: The hidden window that hears lock, unlock, console and remote connect, and resume from sleep, and that receives the deadline timer. A session or power change means key releases may have been missed, so the reducer forgets key state and releases every action; unlock, connect and resume also reinstall the keyboard hook, which Windows can silently drop.
//! Contents: create / destroy — the window; TIMER — the id of the one deadline timer; watcher_proc — its window procedure.

use std::time::Instant;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::RemoteDesktop::{WTSRegisterSessionNotification, WTSUnRegisterSessionNotification, NOTIFY_FOR_THIS_SESSION};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, RegisterClassW, WINDOW_EX_STYLE, WINDOW_STYLE, WM_POWERBROADCAST, WM_TIMER, WNDCLASSW,
};

use super::pump::{reinstall, request_sync, with};

/// The one timer: it fires at the nearest deadline of a recording and is killed when none is left.
pub(super) const TIMER: usize = 1;

const WM_WTSSESSION_CHANGE: u32 = 0x02B1;
const WTS_CONSOLE_CONNECT: usize = 0x1;
const WTS_REMOTE_CONNECT: usize = 0x3;
const WTS_SESSION_LOCK: usize = 0x7;
const WTS_SESSION_UNLOCK: usize = 0x8;
const PBT_APMRESUMESUSPEND: usize = 0x7;
const PBT_APMRESUMEAUTOMATIC: usize = 0x12;

/// Makes the hidden top-level window (broadcast messages such as power changes never reach message-only windows) and registers it for session notifications. Returns None when that fails.
pub(super) fn create() -> Option<HWND> {
    let class: Vec<u16> = "CueInputWatcher\0".encode_utf16().collect();
    let wnd = WNDCLASSW { lpfnWndProc: Some(watcher_proc), lpszClassName: PCWSTR(class.as_ptr()), ..Default::default() };
    unsafe {
        RegisterClassW(&wnd);
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            PCWSTR(class.as_ptr()),
            PCWSTR::null(),
            WINDOW_STYLE::default(),
            0,
            0,
            0,
            0,
            None,
            None,
            None,
            None,
        )
        .ok()?;
        if WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION).is_err() {
            let _ = DestroyWindow(hwnd);
            return None;
        }
        Some(hwnd)
    }
}

/// Removes the window. Must run on the creating thread.
pub(super) fn destroy(hwnd: HWND) {
    unsafe {
        let _ = WTSUnRegisterSessionNotification(hwnd);
        let _ = DestroyWindow(hwnd);
    }
}

/// Window procedure: contained, and the default procedure runs for everything.
unsafe extern "system" fn watcher_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let _ = crate::guard::contain(|| handle(msg, wp.0));
    DefWindowProcW(hwnd, msg, wp, lp)
}

/// Reacts to one message: a timer tick, or a session or power change.
fn handle(msg: u32, wp: usize) {
    match (msg, wp) {
        (WM_TIMER, TIMER) => {
            let _ = with(super::pump::timer_fired);
        }
        (WM_WTSSESSION_CHANGE, WTS_SESSION_LOCK) => reset_keys(false),
        (WM_WTSSESSION_CHANGE, WTS_SESSION_UNLOCK | WTS_CONSOLE_CONNECT | WTS_REMOTE_CONNECT) => reset_keys(true),
        (WM_POWERBROADCAST, PBT_APMRESUMESUSPEND | PBT_APMRESUMEAUTOMATIC) => reset_keys(true),
        _ => {}
    }
}

/// Forgets key state and releases every action, and reinstalls the keyboard hook when Windows may have dropped it.
fn reset_keys(reinstall_hook: bool) {
    let _ = with(|st| {
        st.rt.session(Instant::now());
        if reinstall_hook {
            if let Err(e) = reinstall(st) {
                crate::log::note(&format!("input: {e}"));
            }
        }
    });
    request_sync();
}
