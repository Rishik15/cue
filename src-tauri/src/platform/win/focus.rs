//! Purpose: Which window has the keyboard, and bringing one back to the front before text is sent to it.
//! Contents: foreground_window — identifies the window that has focus; focus_window — bring a window to the front and wait until its control holds the keyboard (logged in `cue.log`).

use std::mem::size_of;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::HWND;
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, GetForegroundWindow, GetGUIThreadInfo, GetWindowThreadProcessId, IsIconic, SetForegroundWindow, ShowWindow,
    GUITHREADINFO, SW_RESTORE,
};

/// How long a window gets to become the foreground (a busy machine activates slowly) and then to have a focused control, before text is sent to it.
const FOCUS_WAIT: Duration = Duration::from_millis(1500);
const FOCUS_SETTLE: Duration = Duration::from_millis(300);
/// Identifies the window that has focus (0 when none), so dictated text is never typed into a different window than the one it was spoken into.
pub fn foreground_window() -> isize {
    unsafe { GetForegroundWindow().0 as isize }
}

/// Windows only lets a process take the foreground when it received the last input. Cue acts on a keyboard hook, which does not count, and a tiling
/// window manager moves windows around too, so a plain `SetForegroundWindow` is often ignored. Sharing the input queue of the current foreground thread
/// for the duration of the call makes the request legitimate; it is the standard workaround and needs no keystroke tricks.
fn force_foreground(window: HWND) {
    // SAFETY: plain Win32 calls on window handles; failures only mean the window stays where it was.
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

/// Brings `window` to the front, gives it the keyboard focus and waits until Windows has done it (used to return to the app text was taken from, and to put the preview in front).
pub fn focus_window(window: isize) -> bool {
    let start = Instant::now();
    let mut asked = start;
    force_foreground(HWND(window as _));
    while foreground_window() != window && start.elapsed() < FOCUS_WAIT {
        std::thread::sleep(Duration::from_millis(10));
        if asked.elapsed() > Duration::from_millis(300) {
            asked = Instant::now();
            force_foreground(HWND(window as _)); // the first request can be swallowed while the other window is still closing
        }
    }
    let there = foreground_window() == window;
    if there {
        wait_for_focused_control(window);
    }
    crate::log::note(&format!("focus: {} after {} ms", if there { "back" } else { "not back" }, start.elapsed().as_millis()));
    there
}

/// The window is in front, but its control may not hold the keyboard yet: keys sent now would be lost. Waits until the window's thread reports a focused control.
fn wait_for_focused_control(window: isize) {
    let start = Instant::now();
    while start.elapsed() < FOCUS_SETTLE {
        let mut info = GUITHREADINFO { cbSize: size_of::<GUITHREADINFO>() as u32, ..Default::default() };
        // SAFETY: reads the GUI state of the thread that owns `window`.
        let known = unsafe { GetGUIThreadInfo(GetWindowThreadProcessId(HWND(window as _), None), &mut info) }.is_ok();
        if known && !info.hwndFocus.is_invalid() {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
