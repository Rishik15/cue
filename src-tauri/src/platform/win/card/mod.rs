//! Purpose: The dictionary card: a native, never-focused, per-pixel-alpha window (no webview) that shows a word's meanings beside the selected text with an arrow pointing at it. Costs one parked thread while hidden; the bitmap exists only while shown.
//! It closes on a second shortcut press or Escape (the caller calls `hide`), and by itself when the word's window loses the front, a click lands outside it or the wheel turns (`watch`, hooks that exist only while the card is up).
//! Contents: init / show / hide — the thread and its messages; open — lay out, place, paint and show; fail / close — say why it did not show, or hide it, and tell the caller.
//! `layout` decides where text goes, `shape` paints the card, arrow, border and shadow, `gdi` has the fonts and the bitmap; colours come from `win::palette`.

mod gdi;
mod layout;
mod shape;
mod watch;

use std::cell::RefCell;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::Mutex;

use windows::core::w;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::*;

use super::palette::{resolve, Palette, Rgb};
use super::theme::system_uses_dark_theme;
use crate::platform::place::{place, Side};
use crate::platform::{CardRequest, Rect};
use gdi::{Dib, Fonts};
use layout::{lay_out, Laid};
use shape::{render, Arrow, Spec};
use watch::Watch;

const SHOW: u32 = WM_APP + 1;
const HIDE: u32 = WM_APP + 2;
/// Sizes in dip: the space around the card for its shadow, the card's corner, the arrow, and how near a corner the arrow may sit.
const MARGIN: f32 = 36.0;
const RADIUS: f32 = 14.0;
const ARROW_HALF: f32 = 11.0;
const ARROW_REACH: f32 = 11.0;
const ARROW_GAP: f32 = 3.0;
const ARROW_INSET: f32 = 26.0;

static HWND_RAW: AtomicIsize = AtomicIsize::new(0);
static REQUEST: Mutex<Option<CardRequest>> = Mutex::new(None);
/// Who to tell when the card goes away. Kept apart from the request so every way out of `open` can still tell it.
static ON_CLOSED: Mutex<Option<fn()>> = Mutex::new(None);

/// What is on screen. Lives on the card thread only.
struct Open {
    _dib: Dib,
    _watch: Watch,
}

thread_local! {
    static STATE: RefCell<Option<Open>> = const { RefCell::new(None) };
}

pub fn init() -> std::io::Result<()> {
    crate::guard::spawn("card", 256 * 1024, run)?;
    Ok(())
}

pub fn show(request: CardRequest) {
    let on_closed = request.on_closed;
    *ON_CLOSED.lock().unwrap_or_else(|e| e.into_inner()) = Some(on_closed);
    *REQUEST.lock().unwrap_or_else(|e| e.into_inner()) = Some(request);
    if !post(SHOW) {
        on_closed(); // no card thread: nothing will ever show, and the caller must not wait for a close
    }
}

/// Closes the card if it is up, and drops a request that has not been shown yet. Cheap: one posted message. False when the card thread is not there to hear it.
pub fn hide() -> bool {
    REQUEST.lock().unwrap_or_else(|e| e.into_inner()).take();
    post(HIDE)
}

fn post_hide() {
    post(HIDE);
}

/// False when the card's window does not exist (yet, or any more).
fn post(msg: u32) -> bool {
    let h = HWND_RAW.load(Ordering::Acquire);
    h != 0 && unsafe { PostMessageW(Some(HWND(h as _)), msg, WPARAM(0), LPARAM(0)) }.is_ok()
}

/// The card could not be shown: say why in the log, then tell the caller.
fn fail(why: &str) {
    crate::log::note(&format!("card: {why}"));
    closed();
}

fn closed() {
    let on_closed = ON_CLOSED.lock().unwrap_or_else(|e| e.into_inner()).take();
    if let Some(on_closed) = on_closed {
        on_closed();
    }
}

fn run() {
    unsafe {
        let Ok(module) = GetModuleHandleW(None) else { return };
        let class = w!("CueCard");
        let wc = WNDCLASSW { lpfnWndProc: Some(wndproc), hInstance: module.into(), lpszClassName: class, ..Default::default() };
        if RegisterClassW(&wc) == 0 {
            return crate::log::note("card: could not register the window class");
        }
        let ex = WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE;
        let Ok(hwnd) = CreateWindowExW(ex, class, w!("Cue Card"), WS_POPUP, 0, 0, 1, 1, None, None, Some(module.into()), None) else {
            return crate::log::note("card: could not create the window");
        };
        HWND_RAW.store(hwnd.0 as isize, Ordering::Release);
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            DispatchMessageW(&msg);
        }
        HWND_RAW.store(0, Ordering::Release);
        closed(); // the thread is ending: whoever waits on the card must not wait for ever
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        SHOW => open(hwnd),
        HIDE => close(hwnd, true),
        // The screen changed under the card (a monitor, the scale, the work area): its place is wrong, so it goes.
        WM_DPICHANGED | WM_DISPLAYCHANGE => close(hwnd, true),
        WM_MOUSEACTIVATE => return LRESULT(MA_NOACTIVATE as isize),
        _ => return DefWindowProcW(hwnd, msg, wp, lp),
    }
    LRESULT(0)
}

fn palette(request: &CardRequest) -> Palette {
    resolve(&request.theme, system_uses_dark_theme())
}

/// DPI scale of the monitor the text is on.
unsafe fn scale_at(target: &Rect) -> f32 {
    let middle = POINT { x: (target.left + target.right) / 2, y: (target.top + target.bottom) / 2 };
    let (mut dpi_x, mut dpi_y) = (96, 96);
    let _ = GetDpiForMonitor(MonitorFromPoint(middle, MONITOR_DEFAULTTONEAREST), MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y);
    dpi_x.max(96) as f32 / 96.0
}

unsafe fn open(hwnd: HWND) {
    close(hwnd, false); // a card already up is replaced: its caller is not told it went away
    let begun = std::time::Instant::now();
    let Some(request) = REQUEST.lock().unwrap_or_else(|e| e.into_inner()).take() else { return closed() }; // hidden before it was shown
    let (scale, palette) = (scale_at(&request.target), palette(&request));
    let fonts = Fonts::new(scale);
    let laid = lay_out(&request.sections, scale, &|text, style| fonts.width(text, style));
    let px = |dip: f32| (dip * scale).round() as i32;
    let placed = place(request.target, request.work, (laid.width, laid.height), px(ARROW_REACH + ARROW_GAP), px(ARROW_INSET));
    let margin = px(MARGIN);
    let Some(dib) = Dib::new(laid.width + 2 * margin, laid.height + 2 * margin) else { return fail("could not create the bitmap") };
    let laid_ms = begun.elapsed().as_millis();
    let alpha = paint(&dib, &laid, placed.side, placed.offset, &palette, margin, scale);
    let painted_ms = begun.elapsed().as_millis();
    fonts.draw(&dib, &laid, margin, &palette);
    dib.restore_alpha(&alpha);
    // Watching for the word's window to lose the front starts before the card shows, and a window that is already not in front means there is nothing to point at any more.
    let bounds = [placed.x, placed.y, placed.x + laid.width, placed.y + laid.height];
    let Some(watch) = Watch::start(bounds, request.source) else { return fail("could not install the hooks that close it") };
    if watch::root(GetForegroundWindow().0 as isize) != watch::root(request.source) {
        return fail("the word's window is no longer in front");
    }
    blit(hwnd, &dib, POINT { x: placed.x - margin, y: placed.y - margin });
    let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
    let _ = SetWindowPos(hwnd, Some(HWND_TOPMOST), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
    STATE.with(|s| *s.borrow_mut() = Some(Open { _dib: dib, _watch: watch }));
    crate::log::note(&format!("card: layout {laid_ms} ms, shape {} ms, shown {} ms", painted_ms - laid_ms, begun.elapsed().as_millis()));
}

/// The card, its arrow, border and shadow into the bitmap; returns the alpha channel to restore after the text is drawn over it.
unsafe fn paint(dib: &Dib, laid: &Laid, side: Side, offset: i32, palette: &Palette, margin: i32, scale: f32) -> Vec<u8> {
    let bytes = |c: Rgb| {
        let (r, g, b) = c.bytes();
        [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0]
    };
    let spec = Spec {
        width: dib.w as usize,
        height: dib.h as usize,
        card: (margin as f32, margin as f32, laid.width as f32, laid.height as f32),
        radius: RADIUS * scale,
        arrow: Some(Arrow { side, offset: offset as f32, half_width: ARROW_HALF * scale, reach: ARROW_REACH * scale }),
        fill: bytes(palette.card),
        border: bytes(palette.border()),
        border_width: scale.max(1.0),
        shadow_dy: 10.0 * scale,
        shadow_blur: 30.0 * scale,
        shadow_alpha: 0.5,
    };
    let mut alpha = Vec::new();
    dib.with_pixels(|pixels| alpha = render(&spec, pixels));
    alpha
}

unsafe fn blit(hwnd: HWND, dib: &Dib, at: POINT) {
    let blend = BLENDFUNCTION { BlendOp: AC_SRC_OVER as u8, BlendFlags: 0, SourceConstantAlpha: 255, AlphaFormat: AC_SRC_ALPHA as u8 };
    let size = SIZE { cx: dib.w, cy: dib.h };
    if let Err(e) =
        UpdateLayeredWindow(hwnd, None, Some(&at), Some(&size), Some(dib.dc), Some(&POINT::default()), COLORREF(0), Some(&blend), ULW_ALPHA)
    {
        crate::log::note(&format!("card: could not update the window: {e}"));
    }
}

/// Hides the card and frees what it held; `notify` tells the caller, except when another card is about to take the place.
unsafe fn close(hwnd: HWND, notify: bool) {
    let Some(open) = STATE.with(|s| s.borrow_mut().take()) else { return };
    let _ = ShowWindow(hwnd, SW_HIDE);
    drop(open); // the hooks come off, the bitmap goes
    if notify {
        closed();
    }
}
