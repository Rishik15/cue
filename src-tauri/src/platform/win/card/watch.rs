//! Purpose: What closes the card by itself while it is up: a click outside it, another program's window coming to the front, the window the word is in being minimised or moved, Alt-Tab, a desktop switch (lock screen, UAC prompt). It also takes the wheel over the card so the card scrolls instead of the window under it.
//! Both hooks exist only while the card is shown, so nothing runs while it is hidden. They only read atomics and post one message to the card's window.
//! Contents: Watch — installs the hooks and removes them when dropped; contains — whether a screen point is on the card; mouse_hook / win_event — the callbacks.

use std::sync::atomic::{AtomicI32, AtomicIsize, Ordering};

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::WindowsAndMessaging::*;

use super::{post_hide, post_scroll};

/// The card's rectangle on the screen and the window the word is in, for the callbacks (which have no access to the card thread's state).
static BOUNDS: [AtomicI32; 4] = [AtomicI32::new(0), AtomicI32::new(0), AtomicI32::new(0), AtomicI32::new(0)];
static SOURCE: AtomicIsize = AtomicIsize::new(0);

/// The top-level window of `window`: apps hosted in a frame (UWP, WinUI) report a child or the frame depending on the call, and both must count as the same window.
pub fn root(window: isize) -> isize {
    unsafe { GetAncestor(HWND(window as _), GA_ROOT).0 as isize }
}

/// Whether the screen point is on the card (not on the soft edge around it).
pub fn contains(x: i32, y: i32) -> bool {
    let [left, top, right, bottom] = std::array::from_fn(|i| BOUNDS[i].load(Ordering::Relaxed));
    x >= left && x < right && y >= top && y < bottom
}

pub struct Watch {
    mouse: HHOOK,
    events: [HWINEVENTHOOK; 3],
}

/// The events watched, as three narrow ranges so the system calls us for nothing else: foreground change; a move or resize beginning; Alt-Tab, minimise and desktop switch (the events between them are ignored by `win_event`).
const RANGES: [(u32, u32); 3] = [
    (EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND),
    (EVENT_SYSTEM_MOVESIZESTART, EVENT_SYSTEM_MOVESIZESTART),
    (EVENT_SYSTEM_SWITCHSTART, EVENT_SYSTEM_DESKTOPSWITCH),
];

impl Watch {
    /// None when a hook could not be installed: the card would then have no way to go away on its own, so the caller does not show it.
    pub unsafe fn start(bounds: [i32; 4], source: isize) -> Option<Watch> {
        bounds.iter().zip(&BOUNDS).for_each(|(v, slot)| slot.store(*v, Ordering::Relaxed));
        SOURCE.store(root(source), Ordering::Relaxed);
        let mouse = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook), None, 0).ok()?;
        let events = RANGES.map(|(from, to)| SetWinEventHook(from, to, None, Some(win_event), 0, 0, WINEVENT_OUTOFCONTEXT));
        let watch = Watch { mouse, events };
        // A hook that failed leaves the others to come off with the drop.
        (!watch.events.iter().any(|e| e.is_invalid())).then_some(watch)
    }
}

impl Drop for Watch {
    fn drop(&mut self) {
        unsafe {
            let _ = UnhookWindowsHookEx(self.mouse);
            self.events.iter().filter(|e| !e.is_invalid()).for_each(|e| {
                let _ = UnhookWinEvent(*e);
            });
        }
    }
}

/// A click outside the card closes it (the click itself still reaches what was clicked); a turn of the wheel over it scrolls it and goes no further, and elsewhere passes untouched.
unsafe extern "system" fn mouse_hook(code: i32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if code >= 0 {
        let event = &*(lp.0 as *const MSLLHOOKSTRUCT);
        match wp.0 as u32 {
            WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_XBUTTONDOWN if !contains(event.pt.x, event.pt.y) => post_hide(),
            WM_MOUSEWHEEL if contains(event.pt.x, event.pt.y) => {
                post_scroll((event.mouseData >> 16) as i16 as i32);
                return LRESULT(1);
            }
            _ => {}
        }
    }
    CallNextHookEx(None, code, wp, lp)
}

/// A window of Cue's own or one that is not shown (a tooltip, an input-method helper) taking the front is not the user leaving the word.
unsafe fn takes_the_front(window: HWND) -> bool {
    let mut pid = 0;
    GetWindowThreadProcessId(window, Some(&mut pid));
    pid != std::process::id() && IsWindowVisible(window).as_bool()
}

/// Focus left the window the word is in, or that window is being minimised or moved, or Alt-Tab or a desktop switch began.
unsafe extern "system" fn win_event(_hook: HWINEVENTHOOK, event: u32, hwnd: HWND, object: i32, _child: i32, _thread: u32, _time: u32) {
    let source = SOURCE.load(Ordering::Relaxed);
    let leaves = match event {
        EVENT_SYSTEM_FOREGROUND => root(hwnd.0 as isize) != source && takes_the_front(hwnd),
        EVENT_SYSTEM_MINIMIZESTART | EVENT_SYSTEM_MOVESIZESTART => object == OBJID_WINDOW.0 && root(hwnd.0 as isize) == source,
        EVENT_SYSTEM_SWITCHSTART | EVENT_SYSTEM_DESKTOPSWITCH => true,
        _ => false,
    };
    if leaves {
        post_hide();
    }
}
