//! File: mod.rs
//! Purpose: The dictionary card: a native, never-focused, per-pixel-alpha window (no webview) that shows a word's meanings beside the selected text with an arrow pointing at it. Its window and thread are made by the first `show` and destroyed when the card is hidden (`host`); the bitmap exists only while shown.
//! The wheel over the card scrolls its text (it is taller than the card's limit for long entries). It closes on a second shortcut press or Escape (the caller calls `hide`), and by itself when the word's window is left for another program, a click lands outside it, or that window is minimised or moved (`watch`, hooks that exist only while the card is up).
//! Contents: show / hide — post to the card thread; open — lay out, place, paint and show; fail / close — say why it did not show, or hide it, and tell the caller.
//! `layout` decides where text goes, `look` and `shape` paint the card, arrow and edge, `gdi` has the fonts and the bitmap, `scroll` repaints the text when it scrolls; colours come from `win::palette`.

mod gdi;
mod layout;
mod look;
mod scroll;
mod shape;
mod watch;

use std::cell::RefCell;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;

use windows::core::w;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;

use super::host::{register_class, Host};
use super::monitor::at_point;
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

static HOST: Host = Host::new("card", run);
/// Which card is up: a scroll posted for one that has since closed or been replaced carries an older number and is dropped.
static SHOWN: AtomicU32 = AtomicU32::new(0);
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

/// Shows the requested card; the thread is started by this call. A second request that arrives before the first has been shown is ignored (its `on_closed` runs at once) and the first is kept. A card that cannot start is reported and `on_closed` runs at once, so the caller never waits for a close; the request is dropped so a later request is not refused. Input: the request.
pub fn show(request: CardRequest) {
    let on_closed = request.on_closed;
    {
        let mut waiting = REQUEST.lock().unwrap_or_else(|e| e.into_inner());
        if waiting.is_some() {
            drop(waiting);
            return on_closed();
        }
        *ON_CLOSED.lock().unwrap_or_else(|e| e.into_inner()) = Some(on_closed);
        *waiting = Some(request);
    }
    if !HOST.post(SHOW, 0, 0, true) {
        REQUEST.lock().unwrap_or_else(|e| e.into_inner()).take();
        closed();
    }
}

/// Closes the card if it is up, and drops a request that has not been shown yet. Cheap: one posted message. False when the card thread is not there to hear it.
pub fn hide() -> bool {
    REQUEST.lock().unwrap_or_else(|e| e.into_inner()).take();
    HOST.post(HIDE, 0, 0, false)
}

/// Asks the card thread to close the card (a hook saw the user leave the word).
fn post_hide() {
    HOST.post(HIDE, 0, 0, false);
}

/// A turn of the wheel over the card, for the card thread to scroll by (`delta` as the wheel reports it, 120 a notch).
fn post_scroll(delta: i32) {
    HOST.post(SCROLL, delta as isize as usize, SHOWN.load(Ordering::Relaxed) as isize, false);
}

/// The card could not be shown: say why in the log and to the user, then tell the caller.
fn fail(why: &str) {
    crate::log::note(&format!("card: {why}"));
    crate::platform::report(&format!("The dictionary card could not open: {why}."));
    closed();
}

/// Tells whoever asked for the card that it is gone (once: the callback is taken).
fn closed() {
    let on_closed = ON_CLOSED.lock().unwrap_or_else(|e| e.into_inner()).take();
    if let Some(on_closed) = on_closed {
        on_closed();
    }
}

/// The card thread could not make its window: the surface is marked failed, the request that waited for it is dropped and its caller is told the card is gone.
fn start_failed(why: &str) {
    HOST.failed(why);
    REQUEST.lock().unwrap_or_else(|e| e.into_inner()).take();
    closed();
}

/// The card thread: makes the window, tells `HOST`, pumps messages until `settle` ends it. The window class stays registered for the process (`host::register_class`). Safety: Win32 window creation and a message loop on this thread, which owns the window until it ends.
fn run() {
    unsafe {
        let Ok(module) = GetModuleHandleW(None) else { return start_failed("no module handle") };
        let class = w!("CueCard");
        let cursor = LoadCursorW(None, IDC_ARROW).unwrap_or_default();
        let wc =
            WNDCLASSW { lpfnWndProc: Some(wndproc), hInstance: module.into(), lpszClassName: class, hCursor: cursor, ..Default::default() };
        if !register_class(&wc) {
            return start_failed("could not register the window class");
        }
        let ex = WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE;
        let created = CreateWindowExW(ex, class, w!("Cue Card"), WS_POPUP, 0, 0, 1, 1, None, None, Some(module.into()), None);
        let Ok(hwnd) = created else {
            return start_failed("could not create the window");
        };
        HOST.ready(hwnd);
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            DispatchMessageW(&msg);
        }
    }
}

/// Ends the card thread when the card is hidden and no message waits: the window is destroyed and the loop quits.
///
/// Must run on the card thread with its window.
unsafe fn settle(hwnd: HWND) {
    if !IsWindowVisible(hwnd).as_bool() && HOST.retire(hwnd) {
        let _ = DestroyWindow(hwnd);
        PostQuitMessage(0);
    }
}

/// A panic must not unwind out of the window procedure (it would abort the app), so it is contained and the message is left unhandled.
unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    crate::guard::contain(|| handle(hwnd, msg, wp, lp)).unwrap_or(LRESULT(0))
}

/// Handles one card message on the card thread. Input: the window and the message. Return: 0 when handled, else the default procedure's answer. The bitmap's margin holds the soft edge. HTTRANSPARENT only reaches windows of this thread, so a click on that margin may still land on the card (which then closes); over the card the pointer is the plain arrow, never a busy cursor. The screen changed under the card (a monitor, the scale, the work area): its place is wrong, so it goes.
unsafe fn handle(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        SHOW => {
            open(hwnd);
            settle(hwnd);
        }
        HIDE => end(hwnd),
        SCROLL => {
            if lp.0 == SHOWN.load(Ordering::Relaxed) as isize {
                scroll(hwnd, wp.0 as isize as i32);
            }
            settle(hwnd);
        }
        WM_DPICHANGED | WM_DISPLAYCHANGE => end(hwnd),
        WM_SETTINGCHANGE if wp.0 == SPI_SETWORKAREA.0 as usize => end(hwnd),
        WM_MOUSEACTIVATE => return LRESULT(MA_NOACTIVATE as isize),
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

/// The colours for the request's theme, resolved now so a Windows light or dark switch is followed.
fn palette(request: &CardRequest) -> Palette {
    resolve(&request.theme, system_uses_dark_theme())
}

/// DPI scale of the monitor the text is on.
fn scale_at(target: &Rect) -> f32 {
    at_point((target.left + target.right) / 2, (target.top + target.bottom) / 2).map_or(1.0, |d| d.scale)
}

/// Closes the card, tells its caller, and lets the thread end.
unsafe fn end(hwnd: HWND) {
    close(hwnd, true);
    settle(hwnd);
}

/// Lays out, places, paints and shows the waiting request, replacing a card already up without telling its caller. Must run on the card thread. With no request waiting it does nothing when a card is up (a second show was ignored) and tells the caller the card is gone when none is (the request was hidden before it was shown). Watching for the word's window to lose the front starts before the card shows, and a window that is already not in front means there is nothing to point at, so the card is not shown. The state is set before anything can send this window a message, so a display change during the blit finds a card to close, and nothing below holds a borrow of it.
unsafe fn open(hwnd: HWND) {
    let begun = std::time::Instant::now();
    let Some(request) = REQUEST.lock().unwrap_or_else(|e| e.into_inner()).take() else {
        if STATE.with(|s| s.borrow().is_none()) {
            closed();
        }
        return;
    };
    close(hwnd, false);
    let scale = scale_at(&request.target);
    let palette = palette(&request);
    let fonts = Fonts::new(scale);
    let px = |dip: f32| (dip * scale).round() as i32;
    let room = (request.work.bottom - request.work.top - 2 * px(MARGIN) - 2 * px(ARROW_INSET)).max(px(40.0));
    let laid = lay_out(&request.title, &request.sections, scale, room, &|text, style| fonts.width(text, style));
    let placed = place(request.target, request.work, (laid.width, laid.height), px(ARROW_REACH + ARROW_GAP), px(ARROW_INSET));
    let margin = px(MARGIN);
    let Some(dib) = Dib::new(laid.width + 2 * margin, laid.height + 2 * margin) else { return fail("could not create the bitmap") };
    let laid_ms = begun.elapsed().as_millis();
    let (card_w, card_h, content_h) = (laid.width, laid.height, laid.content);
    let alpha = paint(&dib, &laid, placed.side, placed.offset, &palette, margin, scale);
    let painted_ms = begun.elapsed().as_millis();
    let at = POINT { x: placed.x - margin, y: placed.y - margin };
    let Some(view) = View::new(fonts, laid, palette, alpha, &dib, at, (margin, scale)) else {
        return fail("could not create the bitmap the text scrolls on");
    };
    let bounds = [placed.x, placed.y, placed.x + card_w, placed.y + card_h];
    let Some(watch) = Watch::start(bounds, request.source, card_h < content_h) else {
        return fail("could not install the hooks that close it");
    };
    if watch::root(GetForegroundWindow().0 as isize) != watch::root(request.source) {
        crate::log::note("card: the word's window is no longer in front");
        return closed();
    }
    let (shot, at) = (Frame::of(&dib), view.at);
    SHOWN.fetch_add(1, Ordering::Relaxed);
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
    /// Copies the bitmap's DC and size out of `dib`.
    fn of(dib: &Dib) -> Frame {
        Frame { dc: dib.dc, w: dib.w, h: dib.h }
    }
}

/// Hands the bitmap in `frame` to the compositor at `at`; a failure is logged and reported. Safety: `hwnd` is the card window and `frame.dc` a live DC owned by the card's state, both used on the card thread only.
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
        crate::platform::report("The dictionary card could not be drawn: Windows refused to update its window.");
    }
}

/// Scrolls the text under a wheel turn and shows the result. The borrow ends before the blit: a message sent during it may close the card. Turns that queued up while the last one was copied are taken together: one copy and one update for all of them, so a fast wheel never falls behind.
unsafe fn scroll(hwnd: HWND, delta: i32) {
    let (mut delta, mut queued) = (delta, MSG::default());
    while PeekMessageW(&mut queued, Some(hwnd), SCROLL, SCROLL, PM_REMOVE).as_bool() {
        if queued.lParam.0 == SHOWN.load(Ordering::Relaxed) as isize {
            delta += queued.wParam.0 as isize as i32;
        }
    }
    let moved = STATE.with(|s| {
        let mut state = s.borrow_mut();
        let open = state.as_mut()?;
        open.view.scroll_by(&open.dib, delta).then(|| (Frame::of(&open.dib), open.view.at))
    });
    if let Some((shot, at)) = moved {
        blit(hwnd, shot, at);
    }
}

/// Hides the card and frees what it held; `notify` tells the caller, except when another card is about to take the place. The hooks come off, the bitmap goes.
unsafe fn close(hwnd: HWND, notify: bool) {
    let Some(open) = STATE.with(|s| s.borrow_mut().take()) else { return };
    let _ = ShowWindow(hwnd, SW_HIDE);
    drop(open);
    if notify {
        closed();
    }
}
