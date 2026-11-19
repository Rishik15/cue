//! File: front_watch.rs
//! Purpose: What closes the dictionary card by itself while it is up and has not taken the keyboard: a click outside it, another program's window coming to the front, the window the word is in being minimised or moved, Alt-Tab, a desktop switch (lock screen, UAC prompt). The hooks live on one small thread per card that exists only while the card is shown, so nothing runs while it is hidden. Everything a watch knows (the card's bounds, the word's window, the "left" flag) lives in thread-local state of its own thread: hook callbacks run on the thread that installed them, so a watch still winding down never sees the next watch's values, and each thread removes its own hooks. A hook only reads that state and posts one quit message to its own thread; the thread then removes the hooks and calls the closer with the card's number.
//! Contents: FrontWatch — the running watch (dropping it ends it); watch — start one; in_front — whether a window is still the one in front; roots / class_and_owner — its top-level window and the front one's, and a window's class and whether it is Cue's, for the log; Local — a watch thread's state; away — whether the window in front means the user left; root / contains — window and point checks; mouse_hook / win_event — the callbacks, each under `guard::contain` so a panic never unwinds into Windows.

use std::cell::Cell;
use std::sync::mpsc;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::platform::Rect;

/// What one watch thread knows, for the callbacks (which have no argument to carry it): the card's rectangle on the screen, the top-level window the word is in, and whether a callback decided the card must go.
#[derive(Clone, Copy)]
struct Local {
    bounds: Rect,
    source: isize,
    left: bool,
}

thread_local! {
    /// This thread's watch; only its own callbacks and its own `run` read it.
    static LOCAL: Cell<Option<Local>> = const { Cell::new(None) };
}

/// A running watch; dropping it asks its thread to end, which removes that thread's hooks. A leave that was already decided still calls the closer, which names the card and so closes only that one.
pub struct FrontWatch {
    thread: u32,
}

impl Drop for FrontWatch {
    /// Asks the thread to end. Safety: a plain post to a thread id; a thread that is gone makes it fail, which is ignored.
    fn drop(&mut self) {
        let _ = unsafe { PostThreadMessageW(self.thread, WM_QUIT, WPARAM(0), LPARAM(0)) };
    }
}

/// The events watched, as three narrow ranges so the system calls us for nothing else: foreground change; a move or resize beginning; Alt-Tab, minimise and desktop switch (the events between them are ignored by `on_event`).
const RANGES: [(u32, u32); 3] = [
    (EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND),
    (EVENT_SYSTEM_MOVESIZESTART, EVENT_SYSTEM_MOVESIZESTART),
    (EVENT_SYSTEM_SWITCHSTART, EVENT_SYSTEM_DESKTOPSWITCH),
];

/// Starts watching for the user leaving card `id`. Input: the card's number, its physical bounds, the window the word is in and what to call (on the watch thread, with the number) when the user has left. Return: None when the thread or a hook could not start: the card would then have no way to go away on its own, so the caller does not show it.
pub fn watch(id: u64, bounds: Rect, source: isize, on_leave: fn(u64)) -> Option<FrontWatch> {
    let (ready, started) = mpsc::channel::<Option<u32>>();
    let spawned = crate::guard::spawn("dict-watch", 64 * 1024, move || run(id, bounds, source, on_leave, &ready));
    spawned.ok()?;
    started.recv().ok().flatten().map(|thread| FrontWatch { thread })
}

/// The watch thread: sets its state, makes its message queue (so a quit posted at once is not lost), installs the hooks, reports its id (None when a hook failed), checks once whether the user already left (a switch before the hooks were in is an event they never hear), pumps messages until quit, removes the hooks and, when a callback or that check asked for it, calls the closer. Safety: the hooks are removed on this thread before it returns.
fn run(id: u64, bounds: Rect, source: isize, on_leave: fn(u64), ready: &mpsc::Sender<Option<u32>>) {
    LOCAL.set(Some(Local { bounds, source: root(source), left: false }));
    let mut message = MSG::default();
    let _ = unsafe { PeekMessageW(&mut message, None, WM_USER, WM_USER, PM_NOREMOVE) };
    let hooks = unsafe { install() };
    let thread = unsafe { GetCurrentThreadId() };
    let _ = ready.send(hooks.is_some().then_some(thread));
    let Some((mouse, events)) = hooks else { return };
    let front = unsafe { GetForegroundWindow() };
    if away(root(front.0 as isize), root(source), unsafe { takes_the_front(front) }) {
        leave();
    }
    while unsafe { GetMessageW(&mut message, None, 0, 0) }.as_bool() {}
    unsafe {
        let _ = UnhookWindowsHookEx(mouse);
        events.iter().filter(|e| !e.is_invalid()).for_each(|e| {
            let _ = UnhookWinEvent(*e);
        });
    }
    if LOCAL.take().is_some_and(|local| local.left) {
        on_leave(id);
    }
}

/// Installs the mouse hook and the three event hooks; None (with any that did install removed) when one failed. Safety: the callbacks are `extern "system"` functions that live for the whole process.
unsafe fn install() -> Option<(HHOOK, [HWINEVENTHOOK; 3])> {
    let mouse = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook), None, 0).ok()?;
    let events = RANGES.map(|(from, to)| SetWinEventHook(from, to, None, Some(win_event), 0, 0, WINEVENT_OUTOFCONTEXT));
    if events.iter().any(|e| e.is_invalid()) {
        let _ = UnhookWindowsHookEx(mouse);
        events.iter().filter(|e| !e.is_invalid()).for_each(|e| {
            let _ = UnhookWinEvent(*e);
        });
        return None;
    }
    Some((mouse, events))
}

/// Tells this thread's loop the user has left: the closer runs once the loop ends. Callbacks run on the thread that installed the hooks, so the quit goes to the calling thread. Safety: a plain post to the current thread id.
fn leave() {
    LOCAL.set(LOCAL.get().map(|local| Local { left: true, ..local }));
    let _ = unsafe { PostThreadMessageW(GetCurrentThreadId(), WM_QUIT, WPARAM(0), LPARAM(0)) };
}

/// Whether `window` (the one the shortcut was pressed in) is still the window in front: the same top-level window. An unknown window (0) is not judged. Input: the window. Return: the answer.
pub fn in_front(window: isize) -> bool {
    window == 0 || root(unsafe { GetForegroundWindow() }.0 as isize) == root(window)
}

/// The top-level window of `window` and of the window in front, for the log line of a failed `in_front`. Input: the window. Return: (its root, the front window's root).
pub fn roots(window: isize) -> (isize, isize) {
    (root(window), root(unsafe { GetForegroundWindow() }.0 as isize))
}

/// The window class of `window` and whether it belongs to Cue's own process, for the log line of a failed `in_front`. Input: the window. Return: (its class, empty for a window that is gone; whether it is Cue's). Safety: plain window queries; the class buffer is a local array, and a stale handle only makes them return nothing.
pub fn class_and_owner(window: isize) -> (String, bool) {
    let hwnd = HWND(window as _);
    let mut buffer = [0u16; 128];
    let len = unsafe { GetClassNameW(hwnd, &mut buffer) }.max(0) as usize;
    let mut pid = 0;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    (String::from_utf16_lossy(&buffer[..len]), pid == std::process::id())
}

/// Whether the window in front means the user left the word's window: another top-level window than `source`, shown and not Cue's own (`foreign`). Input: the front window's root, the word's window's root and whether the front one is foreign. Return: the answer.
fn away(front: isize, source: isize, foreign: bool) -> bool {
    front != source && foreign
}

/// The top-level window of `window`: apps hosted in a frame (UWP, WinUI) report a child or the frame depending on the call, and both must count as the same window. Safety: a stale handle only makes the query return 0.
fn root(window: isize) -> isize {
    unsafe { GetAncestor(HWND(window as _), GA_ROOT).0 as isize }
}

/// Whether the screen point is on this thread's card (not on the soft edge around it).
fn contains(x: i32, y: i32) -> bool {
    LOCAL.get().is_some_and(|Local { bounds: b, .. }| x >= b.left && x < b.right && y >= b.top && y < b.bottom)
}

/// A click outside the card closes it (the click itself still reaches what was clicked).
unsafe extern "system" fn mouse_hook(code: i32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    crate::guard::contain(|| on_mouse(code, wp, lp));
    CallNextHookEx(None, code, wp, lp)
}

/// The body of `mouse_hook`. `lp` must point at the `MSLLHOOKSTRUCT` Windows passes when `code` is not negative.
unsafe fn on_mouse(code: i32, wp: WPARAM, lp: LPARAM) {
    if code < 0 {
        return;
    }
    let event = &*(lp.0 as *const MSLLHOOKSTRUCT);
    let pressed = matches!(wp.0 as u32, WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_XBUTTONDOWN);
    if pressed && !contains(event.pt.x, event.pt.y) {
        leave();
    }
}

/// A window of Cue's own or one that is not shown (a tooltip, an input-method helper) taking the front is not the user leaving the word.
unsafe fn takes_the_front(window: HWND) -> bool {
    let mut pid = 0;
    GetWindowThreadProcessId(window, Some(&mut pid));
    pid != std::process::id() && IsWindowVisible(window).as_bool()
}

/// Focus left the window the word is in, or that window is being minimised or moved, or Alt-Tab or a desktop switch began.
unsafe extern "system" fn win_event(_hook: HWINEVENTHOOK, event: u32, hwnd: HWND, object: i32, _child: i32, _thread: u32, _time: u32) {
    crate::guard::contain(|| on_event(event, hwnd, object));
}

/// The body of `win_event`.
unsafe fn on_event(event: u32, hwnd: HWND, object: i32) {
    let Some(Local { source, .. }) = LOCAL.get() else { return };
    let leaves = match event {
        EVENT_SYSTEM_FOREGROUND => away(root(hwnd.0 as isize), source, takes_the_front(hwnd)),
        EVENT_SYSTEM_MINIMIZESTART | EVENT_SYSTEM_MOVESIZESTART => object == OBJID_WINDOW.0 && root(hwnd.0 as isize) == source,
        EVENT_SYSTEM_SWITCHSTART | EVENT_SYSTEM_DESKTOPSWITCH => true,
        _ => false,
    };
    if leaves {
        leave();
    }
}

#[cfg(test)]
mod tests {
    use super::{away, contains, in_front, Local, LOCAL};
    use crate::platform::Rect;

    /// One watch's card is not another thread's: the bounds one thread sets are invisible to the next, so a watch winding down cannot judge clicks against the next card.
    #[test]
    fn a_watch_threads_state_is_its_own() {
        let card = Rect { left: 0, top: 0, right: 10, bottom: 10 };
        LOCAL.set(Some(Local { bounds: card, source: 0, left: false }));
        assert!(contains(5, 5));
        std::thread::spawn(|| assert!(!contains(5, 5))).join().expect("other thread");
        LOCAL.set(None);
    }

    /// The user left when another shown window that is not Cue's own is in front; the word's own window, or a hidden or Cue window, is not leaving. The same rule judges the foreground event and the check made once the hooks are in.
    #[test]
    fn only_a_foreign_shown_window_in_front_is_leaving() {
        assert!(away(6, 5, true));
        assert!(!away(5, 5, true));
        assert!(!away(6, 5, false));
    }

    /// An unknown word's window is not judged, so a lookup without one is never cancelled for it.
    #[test]
    fn an_unknown_window_counts_as_in_front() {
        assert!(in_front(0));
    }
}
