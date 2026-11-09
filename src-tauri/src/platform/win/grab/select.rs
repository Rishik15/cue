//! File: select.rs
//! Purpose: The region selector: one borderless topmost window over the whole desktop copy, a crosshair, and a drag that lights the chosen rectangle.
//! Contents: pick — run the selector and return the rectangle (None on Escape, right click or a click without a drag); wndproc / handle — mouse, keys and painting; on_move / on_release / on_paint / invalidate — the steps; State — the drag in progress; corners / point / union — rectangle helpers.
//! Runs its own message loop on the calling thread and is gone when it returns. Only the changed part of the window is repainted while dragging.

use std::cell::RefCell;

use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture, VK_ESCAPE};
use windows::Win32::UI::WindowsAndMessaging::*;

use super::shot::Shot;
use crate::platform::focus_window;

/// A drag smaller than this (pixels, each way) is a click, not a selection.
const MIN_SIDE: i32 = 8;

/// The drag in progress, kept per thread for the window procedure.
struct State {
    /// The desktop copy; it outlives the message loop.
    shot: *const Shot,
    /// Where the drag began.
    anchor: Option<(i32, i32)>,
    /// The rectangle now lit.
    lit: Option<RECT>,
    /// The rectangle chosen on release, if big enough.
    chosen: Option<RECT>,
}

thread_local! {
    /// The drag in progress; None outside a selection.
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

/// The rectangle between two corner points, in either order.
fn corners((ax, ay): (i32, i32), (bx, by): (i32, i32)) -> RECT {
    RECT { left: ax.min(bx), top: ay.min(by), right: ax.max(bx), bottom: ay.max(by) }
}

/// The (x, y) packed in a mouse message's `lparam`.
fn point(lparam: LPARAM) -> (i32, i32) {
    ((lparam.0 & 0xFFFF) as i16 as i32, ((lparam.0 >> 16) & 0xFFFF) as i16 as i32)
}

/// The smallest rectangle holding both.
fn union(a: Option<RECT>, b: RECT) -> RECT {
    a.map_or(b, |a| RECT { left: a.left.min(b.left), top: a.top.min(b.top), right: a.right.max(b.right), bottom: a.bottom.max(b.bottom) })
}

/// Runs the selector over `shot` and returns the chosen rectangle (None on Escape, right click, a click without a drag, or when the window cannot be made). The keyboard (Escape) is given to the selector with `focus_window`.
/// Safety: a window class and window owned by this thread, destroyed before returning; `shot` outlives the message loop below.
pub fn pick(shot: &Shot) -> Option<RECT> {
    STATE.set(Some(State { shot, anchor: None, lit: None, chosen: None }));
    unsafe {
        let module = GetModuleHandleW(None).ok()?;
        let class = w!("CueGrabSelector");
        RegisterClassW(&WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: module.into(),
            lpszClassName: class,
            hCursor: LoadCursorW(None, IDC_CROSS).ok()?,
            ..Default::default()
        });
        let window = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            class,
            w!("Cue Grab"),
            WS_POPUP | WS_VISIBLE,
            shot.x,
            shot.y,
            shot.w,
            shot.h,
            None,
            None,
            Some(module.into()),
            None,
        )
        .ok()?;
        focus_window(window.0 as isize);
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        let _ = UnregisterClassW(class, Some(module.into()));
    }
    STATE.take().and_then(|s| s.chosen)
}

/// Asks the selector to close, from any thread: the keyboard hook calls this on Escape, so the selector goes away even when it never got the foreground and its own window never hears the key. Nothing happens when no selector is up. Safety: posts a close message to a window found by class name.
pub fn close() {
    unsafe {
        if let Ok(window) = FindWindowW(w!("CueGrabSelector"), None) {
            let _ = PostMessageW(Some(window), WM_CLOSE, WPARAM(0), LPARAM(0));
        }
    }
}

/// Repaints the area the lit rectangle moved over. Safety: invalidates a rectangle of our own window.
fn invalidate(hwnd: HWND, old: Option<RECT>, new: RECT) {
    let area = union(old, new);
    let _ = unsafe { InvalidateRect(Some(hwnd), Some(&area), false) };
}

/// The mouse moved: the lit rectangle follows the drag.
fn on_move(hwnd: HWND, at: (i32, i32)) {
    let change = STATE.with(|s| {
        let mut state = s.borrow_mut();
        let st = state.as_mut()?;
        let new = corners(st.anchor?, at);
        Some((st.lit.replace(new), new))
    });
    if let Some((old, new)) = change {
        invalidate(hwnd, old, new);
    }
}

/// The button came up: the lit rectangle is chosen if big enough, and the window ends. Safety: ends our own capture and window.
fn on_release(hwnd: HWND) {
    STATE.with(|s| {
        if let Some(st) = s.borrow_mut().as_mut() {
            st.chosen = st.lit.filter(|r| r.right - r.left >= MIN_SIDE && r.bottom - r.top >= MIN_SIDE);
        }
    });
    unsafe {
        let _ = ReleaseCapture();
        let _ = DestroyWindow(hwnd);
    }
}

/// Paints the invalid part of the window from the desktop copy. Safety: standard paint cycle on our own window; the shot outlives the loop.
fn on_paint(hwnd: HWND) {
    unsafe {
        let mut ps = PAINTSTRUCT::default();
        let dc = BeginPaint(hwnd, &mut ps);
        STATE.with(|s| {
            if let Some(st) = s.borrow().as_ref() {
                (*st.shot).paint(dc, ps.rcPaint, st.lit);
            }
        });
        let _ = EndPaint(hwnd, &ps);
    }
}

/// A panic must not unwind out of the window procedure (it would abort the app), so it is contained and the message is left unhandled.
unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    crate::guard::contain(|| handle(hwnd, msg, wparam, lparam)).unwrap_or(LRESULT(0))
}

/// Handles the selector's messages. Switching to another window (Alt+Tab, a notification) ends it, so a full-screen window that nothing can close never stays on top. Safety: only our own window and capture are touched.
unsafe fn handle(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_LBUTTONDOWN => {
            STATE.with(|s| s.borrow_mut().as_mut().map(|st| st.anchor = Some(point(lparam))));
            SetCapture(hwnd);
        }
        WM_MOUSEMOVE => on_move(hwnd, point(lparam)),
        WM_LBUTTONUP => on_release(hwnd),
        WM_RBUTTONDOWN => drop(DestroyWindow(hwnd)),
        WM_DISPLAYCHANGE | WM_DPICHANGED => drop(DestroyWindow(hwnd)),
        WM_ACTIVATE if wparam.0 & 0xFFFF == 0 => drop(DestroyWindow(hwnd)),
        WM_KEYDOWN if wparam.0 == VK_ESCAPE.0 as usize => drop(DestroyWindow(hwnd)),
        WM_PAINT => on_paint(hwnd),
        WM_ERASEBKGND => return LRESULT(1),
        WM_DESTROY => PostQuitMessage(0),
        _ => return DefWindowProcW(hwnd, msg, wparam, lparam),
    }
    LRESULT(0)
}
