//! Purpose: The dictionary card: a native, never-focused, per-pixel-alpha window (no webview) that shows a word's meanings beside the selected text with an arrow pointing at it. Costs one parked thread while hidden; the bitmap exists only while shown.
//! The wheel over the card scrolls its text (it is taller than the card's limit for long entries). It closes on a second shortcut press or Escape (the caller calls `hide`), and by itself when the word's window is left for another program, a click lands outside it, or that window is minimised or moved (`watch`, hooks that exist only while the card is up).
//! Contents: init / show / hide — the thread and its messages; open — lay out, place, paint and show; fail / close — say why it did not show, or hide it, and tell the caller.
//! `layout` decides where text goes, `look` and `shape` paint the card, arrow and edge, `gdi` has the fonts and the bitmap, `scroll` repaints the text when it scrolls; colours come from `win::palette`.

mod gdi;
mod layout;
mod look;
mod scroll;
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

use super::palette::{resolve, Palette};
use super::theme::system_uses_dark_theme;
use crate::platform::place::place;
use crate::platform::{CardRequest, Rect};
use gdi::{Dib, Fonts};
use layout::lay_out;
use scroll::View;
use watch::Watch;

const SHOW: u32 = WM_APP + 1;
const HIDE: u32 = WM_APP + 2;
const SCROLL: u32 = WM_APP + 3;
use look::{paint, ARROW_GAP, ARROW_INSET, ARROW_REACH, MARGIN};

static HWND_RAW: AtomicIsize = AtomicIsize::new(0);
static REQUEST: Mutex<Option<CardRequest>> = Mutex::new(None);
/// Who to tell when the card goes away. Kept apart from the request so every way out of `open` can still tell it.
static ON_CLOSED: Mutex<Option<fn()>> = Mutex::new(None);

/// What is on screen. Lives on the card thread only.
struct Open {
    dib: Dib,
    view: View,
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

/// A turn of the wheel over the card, for the card thread to scroll by (`delta` as the wheel reports it, 120 a notch).
fn post_scroll(delta: i32) {
    let h = HWND_RAW.load(Ordering::Acquire);
    if h != 0 {
        let _ = unsafe { PostMessageW(Some(HWND(h as _)), SCROLL, WPARAM(delta as isize as usize), LPARAM(0)) };
    }
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
        let cursor = LoadCursorW(None, IDC_ARROW).unwrap_or_default();
        let wc =
            WNDCLASSW { lpfnWndProc: Some(wndproc), hInstance: module.into(), lpszClassName: class, hCursor: cursor, ..Default::default() };
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
        SCROLL => scroll(hwnd, wp.0 as isize as i32),
        // The screen changed under the card (a monitor, the scale, the work area): its place is wrong, so it goes.
        WM_DPICHANGED | WM_DISPLAYCHANGE => close(hwnd, true),
        WM_MOUSEACTIVATE => return LRESULT(MA_NOACTIVATE as isize),
        // The bitmap's margin holds the soft edge. HTTRANSPARENT only reaches windows of this thread, so a click on that margin may still land on the card (which then closes); over the card the pointer is the plain arrow, never a busy cursor.
        WM_NCHITTEST => {
            let (x, y) = (lp.0 as i16 as i32, (lp.0 >> 16) as i16 as i32);
            return LRESULT(if watch::contains(x, y) { HTCLIENT as isize } else { HTTRANSPARENT as isize });
        }
        WM_SETCURSOR => {
            SetCursor(LoadCursorW(None, IDC_ARROW).ok());
            return LRESULT(1);
        }
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
    let scale = scale_at(&request.target);
    let palette = palette(&request);
    let fonts = Fonts::new(scale);
    let laid = lay_out(&request.sections, scale, &|text, style| fonts.width(text, style));
    let px = |dip: f32| (dip * scale).round() as i32;
    let placed = place(request.target, request.work, (laid.width, laid.height), px(ARROW_REACH + ARROW_GAP), px(ARROW_INSET));
    let margin = px(MARGIN);
    let Some(dib) = Dib::new(laid.width + 2 * margin, laid.height + 2 * margin) else { return fail("could not create the bitmap") };
    let laid_ms = begun.elapsed().as_millis();
    let (card_w, card_h, content_h) = (laid.width, laid.height, laid.content);
    let alpha = paint(&dib, &laid, placed.side, placed.offset, &palette, margin, scale);
    let painted_ms = begun.elapsed().as_millis();
    let at = POINT { x: placed.x - margin, y: placed.y - margin };
    let view = View::new(fonts, laid, palette, alpha, &dib, at, (margin, (54.0 * scale).round() as i32));
    view.paint(&dib);
    // Watching for the word's window to lose the front starts before the card shows, and a window that is already not in front means there is nothing to point at any more.
    let bounds = [placed.x, placed.y, placed.x + card_w, placed.y + card_h];
    let Some(watch) = Watch::start(bounds, request.source, card_h < content_h) else {
        return fail("could not install the hooks that close it");
    };
    if watch::root(GetForegroundWindow().0 as isize) != watch::root(request.source) {
        return fail("the word's window is no longer in front");
    }
    // The state is set before anything can send this window a message (a display change during the blit must find a card to close), and nothing below holds a borrow of it.
    let (shot, at) = (Frame::of(&dib), view.at);
    STATE.with(|s| *s.borrow_mut() = Some(Open { dib, view, _watch: watch }));
    blit(hwnd, shot, at);
    let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
    let _ = SetWindowPos(hwnd, Some(HWND_TOPMOST), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
    crate::log::note(&format!("card: layout {laid_ms} ms, shape {} ms, shown {} ms", painted_ms - laid_ms, begun.elapsed().as_millis()));
}

/// What `blit` needs from the bitmap, copied out so no borrow of the card's state is held while Windows may send it messages.
#[derive(Clone, Copy)]
struct Frame {
    dc: HDC,
    w: i32,
    h: i32,
}

impl Frame {
    fn of(dib: &Dib) -> Frame {
        Frame { dc: dib.dc, w: dib.w, h: dib.h }
    }
}

unsafe fn blit(hwnd: HWND, frame: Frame, at: POINT) {
    let blend = BLENDFUNCTION { BlendOp: AC_SRC_OVER as u8, BlendFlags: 0, SourceConstantAlpha: 255, AlphaFormat: AC_SRC_ALPHA as u8 };
    let size = SIZE { cx: frame.w, cy: frame.h };
    if let Err(e) = UpdateLayeredWindow(
        hwnd,
        None,
        Some(&at),
        Some(&size),
        Some(frame.dc),
        Some(&POINT::default()),
        COLORREF(0),
        Some(&blend),
        ULW_ALPHA,
    ) {
        crate::log::note(&format!("card: could not update the window: {e}"));
    }
}

/// Scrolls the text under a wheel turn and shows the result.
unsafe fn scroll(hwnd: HWND, delta: i32) {
    // The borrow ends before the blit: a message sent during it may close the card.
    let moved = STATE.with(|s| {
        let mut state = s.borrow_mut();
        let open = state.as_mut()?;
        open.view.scroll_by(&open.dib, delta).then(|| (Frame::of(&open.dib), open.view.at))
    });
    if let Some((shot, at)) = moved {
        blit(hwnd, shot, at);
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
