//! Purpose: Native recording overlay (no webview): a click-through, never-focused, per-pixel-alpha pill with a
//! live level meter, drawn in software. Costs one parked thread while hidden; the bitmap exists only while shown.
//! Contents: show(top) / hide — post messages to the overlay thread; run + wndproc — window and message loop;
//! open / present — placement and compositing. Drawing lives in `canvas`.

mod canvas;

use std::cell::RefCell;
use std::sync::atomic::{AtomicIsize, Ordering};

use windows::core::w;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::*;

use canvas::{Canvas, HEIGHT, WIDTH};

const EDGE_GAP: f32 = 28.0;
const FRAME_MS: u32 = 33;
const SHOW: u32 = WM_APP + 1;
const HIDE: u32 = WM_APP + 2;
const TIMER: usize = 1;

static HWND_RAW: AtomicIsize = AtomicIsize::new(0);

thread_local! { static CANVAS: RefCell<Option<Canvas>> = const { RefCell::new(None) }; }

pub fn init() -> std::io::Result<()> {
    std::thread::Builder::new().name("overlay".into()).stack_size(256 * 1024).spawn(run)?;
    Ok(())
}

pub fn show(top: bool) {
    post(SHOW, top as usize);
}

pub fn hide() {
    post(HIDE, 0);
}

fn post(msg: u32, arg: usize) {
    let h = HWND_RAW.load(Ordering::Acquire);
    if h != 0 {
        let _ = unsafe { PostMessageW(Some(HWND(h as _)), msg, WPARAM(arg), LPARAM(0)) };
    }
}

fn run() {
    unsafe {
        let Ok(module) = GetModuleHandleW(None) else { return };
        let class = w!("CueOverlay");
        let wc = WNDCLASSW { lpfnWndProc: Some(wndproc), hInstance: module.into(), lpszClassName: class, ..Default::default() };
        RegisterClassW(&wc);
        let ex = WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TRANSPARENT;
        let Ok(hwnd) = CreateWindowExW(ex, class, w!("Cue Overlay"), WS_POPUP, 0, 0, 1, 1, None, None, Some(module.into()), None) else {
            return;
        };
        HWND_RAW.store(hwnd.0 as isize, Ordering::Release);
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            DispatchMessageW(&msg);
        }
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        SHOW => {
            open(hwnd, wp.0 == 1);
            LRESULT(0)
        }
        HIDE => {
            let _ = KillTimer(Some(hwnd), TIMER);
            let _ = ShowWindow(hwnd, SW_HIDE);
            CANVAS.with(|c| c.borrow_mut().take()); // Drop frees the bitmap
            LRESULT(0)
        }
        WM_TIMER => {
            CANVAS.with(|c| c.borrow().as_ref().map(|cv| present(hwnd, cv, None)));
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

unsafe fn open(hwnd: HWND, top: bool) {
    let scale = GetDpiForWindow(hwnd).max(96) as f32 / 96.0;
    let (w, h) = ((WIDTH * scale).round() as i32, (HEIGHT * scale).round() as i32);
    let Some(cv) = Canvas::new(w, h, scale) else { return };
    // Top or bottom centre of the work area of the monitor under the cursor.
    let mut cursor = POINT::default();
    let _ = GetCursorPos(&mut cursor);
    let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
    let _ = GetMonitorInfoW(MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST), &mut mi);
    let gap = (EDGE_GAP * scale) as i32;
    let pos =
        POINT { x: (mi.rcWork.left + mi.rcWork.right - w) / 2, y: if top { mi.rcWork.top + gap } else { mi.rcWork.bottom - h - gap } };
    present(hwnd, &cv, Some(pos));
    CANVAS.with(|c| *c.borrow_mut() = Some(cv));
    let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
    let _ = SetWindowPos(hwnd, Some(HWND_TOPMOST), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
    SetTimer(Some(hwnd), TIMER, FRAME_MS, None);
}

/// Paint the current frame and hand it to the compositor; `pos` moves the window (first frame only).
unsafe fn present(hwnd: HWND, cv: &Canvas, pos: Option<POINT>) {
    cv.paint(crate::audio::level());
    let blend = BLENDFUNCTION { BlendOp: AC_SRC_OVER as u8, BlendFlags: 0, SourceConstantAlpha: 255, AlphaFormat: AC_SRC_ALPHA as u8 };
    let size = SIZE { cx: cv.w, cy: cv.h };
    let _ = UpdateLayeredWindow(
        hwnd,
        None,
        pos.as_ref().map(|p| p as *const POINT),
        Some(&size),
        Some(cv.dc),
        Some(&POINT::default()),
        COLORREF(0),
        Some(&blend),
        ULW_ALPHA,
    );
}
