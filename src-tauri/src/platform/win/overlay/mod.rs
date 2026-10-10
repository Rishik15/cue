//! Purpose: Native recording overlay (no webview): a click-through, never-focused, per-pixel-alpha pill in the user's theme. It shows a
//! live level meter that gives way to a one-line caption, typed out as sentences are recognised, which makes the pill grow. Costs one parked thread while hidden;
//! the bitmap exists only while shown.
//! Contents: show / hide / dismiss / caption / notice — post messages to the overlay thread; Overlay — the visible pill's state and one frame (step);
//! run + wndproc — window and message loop; open / open_notice / close / blit — placement and compositing.
//! Drawing lives in `canvas` (pill, bars), `caption` (live text), `notice` (message pill) and `palette` (theme colours).

mod canvas;
mod caption;
mod notice;
mod palette;

use std::cell::RefCell;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::Mutex;

use windows::core::w;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::*;

use super::theme::system_uses_dark_theme;
use crate::platform::Theme;
use canvas::{Canvas, Frame, BASE_WIDTH, HEIGHT};
use caption::Caption;
use palette::{resolve, Palette};

/// Distance from the work area edge. The bottom sits higher so the pill clears the taskbar and rounded screen corners.
const TOP_GAP: f32 = 28.0;
const BOTTOM_GAP: f32 = 64.0;
/// The widest the pill grows for a caption; longer speech scrolls under the left fade.
const MAX_WIDTH: f32 = 520.0;
/// Space between the text and the ends of the pill.
const PAD_X: f32 = 20.0;
const EDGE_FADE: f32 = 24.0;
/// After the final text, the pill waits this long at most for the caption to finish typing before it goes.
const LINGER: std::time::Duration = std::time::Duration::from_millis(1200);
const FRAME_MS: u32 = 33;
/// A message stays long enough to read in one glance.
const NOTICE_MS: u32 = 3500;
const SHOW: u32 = WM_APP + 1;
const HIDE: u32 = WM_APP + 2;
const NOTICE: u32 = WM_APP + 3;
const CAPTION: u32 = WM_APP + 4;
const DISMISS: u32 = WM_APP + 5;
const TIMER: usize = 1;
const NOTICE_TIMER: usize = 2;

static HWND_RAW: AtomicIsize = AtomicIsize::new(0);
static THEME: Mutex<Option<Theme>> = Mutex::new(None);
static NOTICE_TEXT: Mutex<String> = Mutex::new(String::new());
static CAPTION_QUEUE: Mutex<Vec<(usize, String)>> = Mutex::new(Vec::new());

/// What is on screen while dictating. Lives on the overlay thread only.
struct Overlay {
    canvas: Canvas,
    caption: Caption,
    palette: Palette,
    centre_x: i32,
    top: i32,
    /// Set when the dictation is over; the pill goes once the caption has caught up (or after `LINGER`).
    closing_since: Option<std::time::Instant>,
}

thread_local! { static STATE: RefCell<Option<Overlay>> = const { RefCell::new(None) }; }

pub fn init() -> std::io::Result<()> {
    std::thread::Builder::new().name("overlay".into()).stack_size(256 * 1024).spawn(run)?;
    Ok(())
}

/// `top` picks the screen edge; `theme` is the user's Cue theme, read from settings by the caller.
pub fn show(top: bool, theme: Theme) {
    *THEME.lock().unwrap() = Some(theme);
    CAPTION_QUEUE.lock().unwrap().clear();
    post(SHOW, top as usize);
}

/// The dictation is over: the pill goes as soon as its caption has finished typing.
pub fn hide() {
    post(HIDE, 0);
}

/// The pill goes now (Escape, or the speech engine was stopped).
pub fn dismiss() {
    post(DISMISS, 0);
}

/// Sentence number `index` of this dictation, for the live caption. Ignored when the overlay is not showing.
pub fn caption(index: usize, text: &str) {
    CAPTION_QUEUE.lock().unwrap().push((index, text.to_string()));
    post(CAPTION, 0);
}

/// Shows a short message in the overlay's place for a few seconds, then hides it.
pub fn notice(text: &str, top: bool, theme: Theme) {
    *THEME.lock().unwrap() = Some(theme);
    *NOTICE_TEXT.lock().unwrap() = text.to_string();
    post(NOTICE, top as usize);
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
        SHOW => open(hwnd, wp.0 == 1),
        NOTICE => open_notice(hwnd, wp.0 == 1),
        CAPTION => take_captions(),
        HIDE => finish(hwnd),
        DISMISS => close(hwnd),
        WM_TIMER if wp.0 == NOTICE_TIMER => close(hwnd),
        WM_TIMER => STATE.with(|s| s.borrow_mut().as_mut().map(|o| o.step(hwnd))).unwrap_or_default(),
        _ => return DefWindowProcW(hwnd, msg, wp, lp),
    }
    LRESULT(0)
}

fn theme_palette() -> Palette {
    let theme = THEME.lock().unwrap().clone().unwrap_or_default();
    resolve(&theme, system_uses_dark_theme())
}

unsafe fn scale_of(hwnd: HWND) -> f32 {
    GetDpiForWindow(hwnd).max(96) as f32 / 96.0
}

/// Centre and top edge of the pill: top or bottom centre of the work area of the monitor under the cursor.
unsafe fn anchor(h: i32, scale: f32, top: bool) -> (i32, i32) {
    let mut cursor = POINT::default();
    let _ = GetCursorPos(&mut cursor);
    let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
    let _ = GetMonitorInfoW(MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST), &mut mi);
    let y = if top { mi.rcWork.top + (TOP_GAP * scale) as i32 } else { mi.rcWork.bottom - h - (BOTTOM_GAP * scale) as i32 };
    ((mi.rcWork.left + mi.rcWork.right) / 2, y)
}

/// The dictation ended: close now, or let the caption finish typing first so it never cuts off mid-word.
unsafe fn finish(hwnd: HWND) {
    let waiting = STATE.with(|s| {
        let mut state = s.borrow_mut();
        let Some(o) = state.as_mut().filter(|o| !o.caption.is_empty() && !o.caption.typing_done()) else { return false };
        o.closing_since.get_or_insert_with(std::time::Instant::now);
        true
    });
    if !waiting {
        close(hwnd);
    }
}

unsafe fn close(hwnd: HWND) {
    let _ = KillTimer(Some(hwnd), TIMER);
    let _ = KillTimer(Some(hwnd), NOTICE_TIMER);
    let _ = ShowWindow(hwnd, SW_HIDE);
    STATE.with(|s| s.borrow_mut().take()); // Drop frees the bitmap and the caption font
}

unsafe fn open(hwnd: HWND, top: bool) {
    let _ = KillTimer(Some(hwnd), NOTICE_TIMER);
    let scale = scale_of(hwnd);
    let h = (HEIGHT * scale).round() as i32;
    let Some(canvas) = Canvas::new((MAX_WIDTH * scale).round() as i32, h, scale) else { return };
    let (centre_x, y) = anchor(h, scale, top);
    let mut overlay = Overlay { canvas, caption: Caption::new(scale), palette: theme_palette(), centre_x, top: y, closing_since: None };
    overlay.step(hwnd);
    let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
    let _ = SetWindowPos(hwnd, Some(HWND_TOPMOST), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
    STATE.with(|s| *s.borrow_mut() = Some(overlay));
    SetTimer(Some(hwnd), TIMER, FRAME_MS, None);
}

/// A notice is painted once; the window keeps its pixels, so no bitmap or timer is needed afterwards except the one that hides it.
unsafe fn open_notice(hwnd: HWND, top: bool) {
    let _ = KillTimer(Some(hwnd), TIMER);
    STATE.with(|s| s.borrow_mut().take());
    let (scale, text, palette) = (scale_of(hwnd), NOTICE_TEXT.lock().unwrap().clone(), theme_palette());
    let font = notice::font(scale);
    let h = (HEIGHT * scale).round() as i32;
    let w = notice::width(&text, font, scale, (BASE_WIDTH * scale).round() as i32);
    if let Some(mut canvas) = Canvas::new(w, h, scale) {
        canvas.paint(&Frame { width: w, level: 0.0, live: false, bars_alpha: 0.0, palette: &palette });
        let pad = (notice::PAD_X * scale) as i32;
        let mut area = RECT { left: pad, top: 0, right: w - pad, bottom: h };
        notice::draw(canvas.dc, &text, font, &mut area, palette.ink);
        canvas.opaque(&RECT { left: pad, top: h / 5, right: w - pad, bottom: h - h / 5 });
        let (centre_x, y) = anchor(h, scale, top);
        blit(hwnd, &canvas, w, POINT { x: centre_x - w / 2, y });
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        let _ = SetWindowPos(hwnd, Some(HWND_TOPMOST), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
        SetTimer(Some(hwnd), NOTICE_TIMER, NOTICE_MS, None);
    }
    let _ = DeleteObject(font.into());
}

unsafe fn take_captions() {
    let queued = std::mem::take(&mut *CAPTION_QUEUE.lock().unwrap());
    STATE.with(|s| {
        if let Some(o) = s.borrow_mut().as_mut() {
            queued.iter().for_each(|(index, text)| o.caption.set(*index, text));
        }
    });
}

impl Overlay {
    /// The width the pill is heading for: its resting width, or as wide as the typed text needs up to `MAX_WIDTH`.
    unsafe fn target_width(&self) -> f32 {
        let s = self.canvas.scale;
        let base = BASE_WIDTH * s;
        if self.caption.is_empty() {
            return base;
        }
        (2.0 * PAD_X * s + self.caption.visible_width() as f32 + caption::CARET_ROOM * s).clamp(base, MAX_WIDTH * s)
    }

    fn text_area(&self, width: i32) -> RECT {
        let (s, h) = (self.canvas.scale, self.canvas.h);
        RECT { left: (PAD_X * s) as i32, top: h / 5, right: width - (PAD_X * s) as i32, bottom: h - h / 5 }
    }

    /// One frame: type a little more, size the pill to the text, paint pill and bars (until text takes over), draw the caption, hand it over.
    unsafe fn step(&mut self, hwnd: HWND) {
        self.caption.advance();
        let w = self.target_width().round() as i32; // typing is the animation: the pill simply follows the text
        let frame = Frame {
            width: w,
            level: crate::audio::level(),
            live: crate::audio::live(),
            bars_alpha: 1.0 - self.caption.presence(),
            palette: &self.palette,
        };
        self.canvas.paint(&frame);
        if !self.caption.is_empty() {
            let area = self.text_area(w);
            if self.caption.draw(self.canvas.dc, &area, &self.palette, self.canvas.scale) {
                self.canvas.fade_left(&area, self.palette.card, (EDGE_FADE * self.canvas.scale) as i32);
            }
            self.canvas.opaque(&area);
        }
        blit(hwnd, &self.canvas, w, POINT { x: self.centre_x - w / 2, y: self.top });
        if self.closing_since.is_some_and(|at| self.caption.typing_done() || at.elapsed() > LINGER) {
            close(hwnd);
        }
    }
}

/// Hand the left `width` pixels of an already painted canvas to the compositor at `pos`.
unsafe fn blit(hwnd: HWND, cv: &Canvas, width: i32, pos: POINT) {
    let blend = BLENDFUNCTION { BlendOp: AC_SRC_OVER as u8, BlendFlags: 0, SourceConstantAlpha: 255, AlphaFormat: AC_SRC_ALPHA as u8 };
    let size = SIZE { cx: width, cy: cv.h };
    let _ = UpdateLayeredWindow(
        hwnd,
        None,
        Some(&pos),
        Some(&size),
        Some(cv.dc),
        Some(&POINT::default()),
        COLORREF(0),
        Some(&blend),
        ULW_ALPHA,
    );
}
