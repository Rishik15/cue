//! File: focus.rs
//! Purpose: Which window has the keyboard, and bringing one back to the front before text is sent to it.
//! Contents: target_now / target_in / foreground / capture_target — the window, process, thread and focused element keys would go to; foreground_window — identifies the window that has focus; focus_window — bring a window to the front and wait until its control holds the keyboard (logged in `cue.log`).

use std::mem::size_of;
use std::time::{Duration, Instant};

use crate::platform::Target;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, GetClassNameW, GetForegroundWindow, GetGUIThreadInfo, GetWindowThreadProcessId, IsIconic, SetForegroundWindow,
    ShowWindow, GUITHREADINFO, SW_RESTORE,
};

/// How long a window gets to become the foreground (a busy machine activates slowly) and then to have a focused control, before text is sent to it.
const FOCUS_WAIT: Duration = Duration::from_millis(1500);
/// How long a window that is in front gets to report a focused control.
const FOCUS_SETTLE: Duration = Duration::from_millis(300);

/// Identifies the window that has focus (0 when none), so dictated text is never typed into a different window than the one it was spoken into.
/// Safety: a plain handle query.
pub fn foreground_window() -> isize {
    unsafe { GetForegroundWindow().0 as isize }
}

/// Windows only lets a process take the foreground when it received the last input. Cue acts on a keyboard hook, which does not count, and a tiling
/// window manager moves windows around too, so a plain `SetForegroundWindow` is often ignored. Sharing the input queue of the current foreground thread
/// for the duration of the call makes the request legitimate; it is the standard workaround and needs no keystroke tricks.
/// Safety: plain Win32 calls on window handles; failures only mean the window stays where it was.
fn force_foreground(window: HWND) {
    unsafe {
        if IsIconic(window).as_bool() {
            let _ = ShowWindow(window, SW_RESTORE);
        }
        let (mine, theirs) = (GetCurrentThreadId(), GetWindowThreadProcessId(GetForegroundWindow(), None));
        let attached = theirs != 0 && theirs != mine && AttachThreadInput(mine, theirs, true).as_bool();
        let _ = BringWindowToTop(window);
        let _ = SetForegroundWindow(window);
        if attached {
            let _ = AttachThreadInput(mine, theirs, false);
        }
    }
}

/// Brings `window` to the front, gives it the keyboard focus and waits until Windows has done it (used to return to the app text was taken from, and to put the preview in front). The first request can be swallowed while the other window is still closing, so it is repeated every 300 ms. True when the window is in front.
pub fn focus_window(window: isize) -> bool {
    let start = Instant::now();
    let mut asked = start;
    force_foreground(HWND(window as _));
    while foreground_window() != window && start.elapsed() < FOCUS_WAIT {
        std::thread::sleep(Duration::from_millis(10));
        if asked.elapsed() > Duration::from_millis(300) {
            asked = Instant::now();
            force_foreground(HWND(window as _));
        }
    }
    let there = foreground_window() == window;
    if there {
        wait_for_focused_control(window);
    }
    crate::log::note(&format!("focus: {} after {} ms", if there { "back" } else { "not back" }, start.elapsed().as_millis()));
    there
}

/// The window is in front, but its control may not hold the keyboard yet: keys sent now would be lost. Waits until the window's thread reports a focused control. Safety: reads the GUI state of the thread that owns `window`.
fn wait_for_focused_control(window: isize) {
    let start = Instant::now();
    while start.elapsed() < FOCUS_SETTLE {
        let mut info = GUITHREADINFO { cbSize: size_of::<GUITHREADINFO>() as u32, ..Default::default() };
        let known = unsafe { GetGUIThreadInfo(GetWindowThreadProcessId(HWND(window as _), None), &mut info) }.is_ok();
        if known && !info.hwndFocus.is_invalid() {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// The place keys would go to right now: the foreground window, its class, process and thread, and (only when `with_element`) the focused element's UI Automation identity. All zeros and empty for no foreground window.
pub fn target_now(with_element: bool) -> Target {
    let mut target = foreground();
    if with_element && target.window != 0 {
        if let Some(field) = super::context::focused_field() {
            (target.runtime_id, target.key) = (Some(field.runtime_id), field.key);
        }
    }
    target
}

/// `target_now` with the focused element read on the caller's `session` (kept across a loop that asks per character), or without one when there is no session.
pub fn target_in(session: Option<&super::context::Session>) -> Target {
    let mut target = foreground();
    if let (Some(session), true) = (session, target.window != 0) {
        if let Some(field) = super::context::focused_field_in(session) {
            (target.runtime_id, target.key) = (Some(field.runtime_id), field.key);
        }
    }
    target
}

/// The foreground window, its class, process and thread, with no element. All zeros and empty for no foreground window. Safety: plain window queries; the class buffer is a local array.
pub fn foreground() -> Target {
    let window = unsafe { GetForegroundWindow() };
    if window.is_invalid() {
        return Target::default();
    }
    let mut pid = 0;
    let thread = unsafe { GetWindowThreadProcessId(window, Some(&mut pid)) };
    let mut buffer = [0u16; 128];
    let len = unsafe { GetClassNameW(window, &mut buffer) }.max(0) as usize;
    Target { window: window.0 as isize, class: String::from_utf16_lossy(&buffer[..len]), pid, thread, ..Target::default() }
}

/// The token for the place the text is for, taken at the start of a flow, before any wait: window, class, process, thread and the focused element's runtime id when UI Automation gives one.
pub fn capture_target() -> Target {
    target_now(true)
}
