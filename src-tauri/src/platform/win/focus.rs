//! File: focus.rs
//! Purpose: Which window has the keyboard, and bringing one back to the front before text is sent to it.
//! Contents: target_now / foreground / capture_target — the window, process, thread and focused element keys would go to; window_owner — the process and thread that own a window now; foreground_window — identifies the window that has focus; raise_window — ask once for a window to be the foreground, without waiting on it; focus_window — bring a window to the front and wait until its control holds the keyboard (logged in `cue.log`).

use std::mem::size_of;
use std::time::{Duration, Instant};

use crate::platform::Target;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::UI::WindowsAndMessaging::{
    GetClassNameW, GetForegroundWindow, GetGUIThreadInfo, GetWindowThreadProcessId, IsIconic, SetForegroundWindow, SetWindowPos,
    ShowWindowAsync, GUITHREADINFO, HWND_TOP, SWP_ASYNCWINDOWPOS, SWP_NOMOVE, SWP_NOSIZE, SW_RESTORE,
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
/// Nothing here waits on `window`'s thread, so a hung app cannot hold up the caller: the restore and the move to the top are posted to that thread
/// (`ShowWindowAsync`: "without waiting for the operation to complete"; `SetWindowPos` with `SWP_ASYNCWINDOWPOS`: "the system posts the request to the
/// thread that owns the window. This prevents the calling thread from blocking", which needs different input queues, so both go out before the attach),
/// https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-showwindowasync and
/// https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowpos . The synchronous `ShowWindow` and `BringWindowToTop` blocked
/// 3 s on a window whose thread slept 3 s (`tests::a_hung_window_does_not_block_the_raise`).
/// Safety: plain Win32 calls on window handles; failures only mean the window stays where it was.
fn force_foreground(window: HWND) {
    unsafe {
        if IsIconic(window).as_bool() {
            let _ = ShowWindowAsync(window, SW_RESTORE);
        }
        let _ = SetWindowPos(window, Some(HWND_TOP), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_ASYNCWINDOWPOS);
        let (mine, theirs) = (GetCurrentThreadId(), GetWindowThreadProcessId(GetForegroundWindow(), None));
        let attached = theirs != 0 && theirs != mine && AttachThreadInput(mine, theirs, true).as_bool();
        let _ = SetForegroundWindow(window);
        if attached {
            let _ = AttachThreadInput(mine, theirs, false);
        }
    }
}

/// Asks once for `window` to become the foreground and returns at once, without waiting on `window`'s thread (used on a thread that must not wait, by a window that holds the foreground and gives it back). Input: the window handle.
pub fn raise_window(window: isize) {
    force_foreground(HWND(window as _));
}

/// Brings `window` to the front, gives it the keyboard focus and waits until Windows has done it (used to return to the app text was taken from, and to put the preview in front). The first request can be swallowed while the other window is still closing, so it is repeated every 300 ms. True when the window is in front.
/// shortcut: polls the foreground every 10 ms for at most `FOCUS_WAIT` (1.5 s), only while a paste returns focus; upgrade to an `EVENT_SYSTEM_FOREGROUND` hook if that wait ever shows in a bench.
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
/// shortcut: polls every 10 ms for at most `FOCUS_SETTLE` (300 ms), only after a focus change; Windows sends no event when another thread's control takes the keyboard, so no upgrade short of a per-app hook.
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

/// The foreground window, its class, process and thread, with no element. All zeros and empty for no foreground window. Safety: plain window queries; the class buffer is a local array.
pub fn foreground() -> Target {
    let window = unsafe { GetForegroundWindow() };
    if window.is_invalid() {
        return Target::default();
    }
    let mut buffer = [0u16; 128];
    let len = unsafe { GetClassNameW(window, &mut buffer) }.max(0) as usize;
    Target { class: String::from_utf16_lossy(&buffer[..len]), ..window_owner(window.0 as isize) }
}

/// The window with the process and thread that own it now, so a kept handle can be checked before use (Windows reuses the handles of closed windows). Input: the window handle. Return: the window, and process and thread 0 when it no longer exists. Safety: a plain handle query; a stale handle only returns 0.
pub fn window_owner(window: isize) -> Target {
    let mut pid = 0;
    let thread = unsafe { GetWindowThreadProcessId(HWND(window as _), Some(&mut pid)) };
    Target { window, pid, thread, ..Target::default() }
}

/// The token for the place the text is for, taken at the start of a flow, before any wait: window, class, process, thread and the focused element's runtime id when UI Automation gives one, whether that element was already a menu-bar item, and the moment (`menu::leave_for` needs both).
pub fn capture_target() -> Target {
    let now = target_now(true);
    Target { menu_open: super::menu::is_bar_item(&now.key), pressed: Some(Instant::now()), ..now }
}

#[cfg(test)]
mod tests {
    use super::raise_window;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};
    use windows::core::w;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_POPUP, WS_VISIBLE,
    };

    /// How long the stand-in app stops answering its messages.
    const HUNG: Duration = Duration::from_secs(3);

    /// A window whose thread stops answering (a hung app) does not hold up the raise: it returns long before that thread answers again. Safety: the stand-in window is a system `STATIC` window, created, left unanswered and destroyed on its own thread.
    #[test]
    fn a_hung_window_does_not_block_the_raise() {
        let (send, made) = mpsc::channel();
        let hung = std::thread::spawn(move || {
            let window = unsafe {
                CreateWindowExW(
                    WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                    w!("STATIC"),
                    w!(""),
                    WS_POPUP | WS_VISIBLE,
                    -100,
                    -100,
                    1,
                    1,
                    None,
                    None,
                    None,
                    None,
                )
            };
            let _ = send.send(window.as_ref().map(|h| h.0 as isize).unwrap_or(0));
            std::thread::sleep(HUNG);
            if let Ok(window) = window {
                let _ = unsafe { DestroyWindow(window) };
            }
        });
        let window = made.recv().unwrap();
        assert_ne!(window, 0, "the stand-in window was not created");
        let start = Instant::now();
        raise_window(window);
        let took = start.elapsed();
        let _ = hung.join();
        assert!(took < HUNG / 6, "the raise waited {took:?} on a hung window");
    }
}
