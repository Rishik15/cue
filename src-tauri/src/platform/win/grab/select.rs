//! Purpose: The region selector: one borderless topmost window over the whole desktop copy, a crosshair, and a drag that lights the chosen rectangle.
//! Contents: pick — run the selector and return the rectangle (None on Escape, right click or a click without a drag); wndproc — mouse, keys and painting.
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

struct State {
    shot: *const Shot,
    anchor: Option<(i32, i32)>,
    lit: Option<RECT>,
    chosen: Option<RECT>,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

fn corners((ax, ay): (i32, i32), (bx, by): (i32, i32)) -> RECT {
    RECT { left: ax.min(bx), top: ay.min(by), right: ax.max(bx), bottom: ay.max(by) }
}

fn point(lparam: LPARAM) -> (i32, i32) {
    ((lparam.0 & 0xFFFF) as i16 as i32, ((lparam.0 >> 16) & 0xFFFF) as i16 as i32)
}

fn union(a: Option<RECT>, b: RECT) -> RECT {
    a.map_or(b, |a| RECT { left: a.left.min(b.left), top: a.top.min(b.top), right: a.right.max(b.right), bottom: a.bottom.max(b.bottom) })
}

pub fn pick(shot: &Shot) -> Option<RECT> {
    STATE.set(Some(State { shot, anchor: None, lit: None, chosen: None }));
    // SAFETY: a window class and window owned by this thread, destroyed before returning; `shot` outlives the message loop below.
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
        focus_window(window.0 as isize); // the keyboard (Escape) goes to the selector
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        let _ = UnregisterClassW(class, Some(module.into()));
    }
    STATE.take().and_then(|s| s.chosen)
}

fn invalidate(hwnd: HWND, old: Option<RECT>, new: RECT) {
    let area = union(old, new);
    // SAFETY: invalidating a rectangle of our own window.
    let _ = unsafe { InvalidateRect(Some(hwnd), Some(&area), false) };
}

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

fn on_release(hwnd: HWND) {
    STATE.with(|s| {
        if let Some(st) = s.borrow_mut().as_mut() {
            st.chosen = st.lit.filter(|r| r.right - r.left >= MIN_SIDE && r.bottom - r.top >= MIN_SIDE);
        }
    });
    // SAFETY: ends our own capture and window.
    unsafe {
        let _ = ReleaseCapture();
        let _ = DestroyWindow(hwnd);
    }
}

fn on_paint(hwnd: HWND) {
    // SAFETY: standard paint cycle on our own window; the shot outlives the loop.
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

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_LBUTTONDOWN => {
            STATE.with(|s| s.borrow_mut().as_mut().map(|st| st.anchor = Some(point(lparam))));
            SetCapture(hwnd);
        }
        WM_MOUSEMOVE => on_move(hwnd, point(lparam)),
        WM_LBUTTONUP => on_release(hwnd),
        WM_RBUTTONDOWN => drop(DestroyWindow(hwnd)),
        // Switching to another window (Alt+Tab, a notification) must not leave a full-screen window on top that nothing can close.
        WM_ACTIVATE if wparam.0 & 0xFFFF == 0 => drop(DestroyWindow(hwnd)),
        WM_KEYDOWN if wparam.0 == VK_ESCAPE.0 as usize => drop(DestroyWindow(hwnd)),
        WM_PAINT => on_paint(hwnd),
        WM_ERASEBKGND => return LRESULT(1),
        WM_DESTROY => PostQuitMessage(0),
        _ => return DefWindowProcW(hwnd, msg, wparam, lparam),
    }
    LRESULT(0)
}
