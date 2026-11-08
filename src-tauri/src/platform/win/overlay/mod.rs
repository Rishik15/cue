//! File: mod.rs
//! Purpose: Native recording overlay (no webview): a click-through, never-focused, per-pixel-alpha pill in the user's theme. It shows a
//! live level meter that gives way to a one-line caption, typed out as sentences are recognised, which makes the pill grow. Its window and thread exist only while it has something to show (`host`): the first `show` or `notice` makes them, going hidden destroys them; the bitmap exists only while shown.
//! Contents: Overlay — the visible pill's state and one frame (step);
//! run + wndproc — window and message loop; open / open_notice / close / blit — placement and compositing.
//! The public calls and the pill's owner live in `mailbox`. Drawing lives in `canvas` (pill, bars), `caption` (live text) and `notice` (message pill); colours come from `win::palette`.

mod canvas;
mod caption;
mod mailbox;
mod notice;
mod place;

pub use mailbox::{caption, dismiss, hide, init, notice, show};

use std::cell::RefCell;

use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;

use super::fonts::Private;
use super::host::register_class;
use super::palette::{resolve, Palette};
use super::theme::system_uses_dark_theme;
use canvas::{Canvas, Frame, BASE_WIDTH, HEIGHT};
use caption::Caption;
use mailbox::{notice_text, owner, take_captions, theme, CAPTION, DISMISS, HIDE, HOST, NOTICE, SHOW};
use place::{anchor, animations_on, blit, drew, scale_here};

/// The widest the pill grows for a caption; longer speech scrolls under the left fade.
const MAX_WIDTH: f32 = 680.0;
/// Space between the text and the ends of the pill.
const PAD_X: f32 = 20.0;
const EDGE_FADE: f32 = 16.0;
/// After the final text, the pill waits this long at most for the caption to finish typing before it goes.
const LINGER: std::time::Duration = std::time::Duration::from_millis(1200);
const FRAME_MS: u32 = 33;
/// A message stays long enough to read in one glance.
const NOTICE_MS: u32 = 3500;
const TIMER: usize = 1;
const NOTICE_TIMER: usize = 2;

/// A short phrase shows only the bars; once a recording has run this long the recognised text replaces them.
const CAPTION_AFTER: std::time::Duration = std::time::Duration::from_secs(6);

/// What is on screen while dictating. Lives on the overlay thread only.
struct Overlay {
    canvas: Canvas,
    caption: Caption,
    palette: Palette,
    centre_x: i32,
    top: i32,
    at_top: bool,
    shown_at: std::time::Instant,
    /// Set when the dictation is over; the pill goes once the caption has caught up (or after `LINGER`).
    closing_since: Option<std::time::Instant>,
}

thread_local! { static STATE: RefCell<Option<Overlay>> = const { RefCell::new(None) }; }

/// The overlay thread: makes the window, tells `HOST`, pumps messages until `settle` ends it. The window class stays registered for the process (`host::register_class`). Safety: Win32 window creation and a message loop on this thread, which owns the window until it ends.
fn run() {
    unsafe {
        let Ok(module) = GetModuleHandleW(None) else { return HOST.failed("no module handle") };
        let class = w!("CueOverlay");
        let wc = WNDCLASSW { lpfnWndProc: Some(wndproc), hInstance: module.into(), lpszClassName: class, ..Default::default() };
        if !register_class(&wc) {
            return HOST.failed("could not register the window class");
        }
        let ex = WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TRANSPARENT;
        let created = CreateWindowExW(ex, class, w!("Cue Overlay"), WS_POPUP, 0, 0, 1, 1, None, None, Some(module.into()), None);
        let Ok(hwnd) = created else {
            return HOST.failed("could not create the window");
        };
        HOST.ready(hwnd);
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            DispatchMessageW(&msg);
        }
    }
}

/// Ends the overlay thread when nothing is on screen and no message waits: the window is destroyed and the loop quits.
///
/// Must run on the overlay thread with its window.
unsafe fn settle(hwnd: HWND) {
    if !IsWindowVisible(hwnd).as_bool() && HOST.retire(hwnd) {
        let _ = DestroyWindow(hwnd);
        PostQuitMessage(0);
    }
}

/// Hides the pill or message and lets the thread end.
unsafe fn end(hwnd: HWND) {
    close(hwnd);
    settle(hwnd);
}

/// A panic must not unwind out of the window procedure (it would abort the app), so it is contained and the message is left unhandled.
unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    crate::guard::contain(|| handle(hwnd, msg, wp, lp)).unwrap_or(LRESULT(0))
}

/// Handles one overlay message on the overlay thread. Input: the window and the message. Return: 0 when handled, else the default procedure's answer.
unsafe fn handle(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        SHOW => {
            open(hwnd, wp.0 == 1);
            settle(hwnd);
        }
        NOTICE => {
            open_notice(hwnd, wp.0 == 1);
            settle(hwnd);
        }
        CAPTION => add_captions(),
        HIDE if wp.0 as u64 == owner() => finish(hwnd),
        HIDE => settle(hwnd),
        WM_DISPLAYCHANGE => relayout(hwnd),
        WM_SETTINGCHANGE if wp.0 == SPI_SETWORKAREA.0 as usize => relayout(hwnd),
        DISMISS => end(hwnd),
        WM_TIMER if wp.0 == NOTICE_TIMER => end(hwnd),
        WM_TIMER => tick(hwnd),
        _ => return DefWindowProcW(hwnd, msg, wp, lp),
    }
    LRESULT(0)
}

/// `close` takes the state again, so it must run after the frame's borrow ends.
unsafe fn tick(hwnd: HWND) {
    if STATE.with(|s| s.borrow_mut().as_mut().is_some_and(|o| o.step(hwnd))) {
        end(hwnd);
    }
}

/// The colours for the user's theme, resolved now so a Windows light or dark switch is followed.
fn theme_palette() -> Palette {
    let theme = theme();
    resolve(&theme, system_uses_dark_theme())
}

/// The dictation ended: close now, or let the caption finish typing first so it never cuts off mid-word. No state means a notice (or nothing) is on screen: it keeps its own timer, so a late HIDE must not wipe the message.
unsafe fn finish(hwnd: HWND) {
    let Some(waiting) = STATE.with(|s| {
        let mut state = s.borrow_mut();
        let o = state.as_mut()?;
        let waiting = o.text_visible() && !o.caption.typing_done();
        if waiting {
            o.closing_since.get_or_insert_with(std::time::Instant::now);
        }
        Some(waiting)
    }) else {
        return settle(hwnd);
    };
    if !waiting {
        end(hwnd);
    }
}

/// The screens or the work area changed while the pill is up (a monitor was unplugged, the resolution, scale or taskbar changed): the pill opens again where the pointer is now, at the new scale. The caption typed so far starts over, and a pill already waiting to close keeps waiting. A notice has no state and keeps its own timer. A message has no state to redraw from: it goes, as a card does.
unsafe fn relayout(hwnd: HWND) {
    let Some((at_top, closing_since)) = STATE.with(|s| s.borrow().as_ref().map(|o| (o.at_top, o.closing_since))) else {
        if IsWindowVisible(hwnd).as_bool() {
            end(hwnd);
        }
        return;
    };
    close(hwnd);
    open(hwnd, at_top);
    STATE.with(|s| s.borrow_mut().as_mut().map(|o| o.closing_since = closing_since));
}

/// Hides the window, stops both timers and frees the pill's bitmap and caption font. Must run on the overlay thread. Drop frees the bitmap and the caption font.
unsafe fn close(hwnd: HWND) {
    let _ = KillTimer(Some(hwnd), TIMER);
    let _ = KillTimer(Some(hwnd), NOTICE_TIMER);
    let _ = ShowWindow(hwnd, SW_HIDE);
    STATE.with(|s| s.borrow_mut().take());
}

/// Paints and shows the recording pill at the top or bottom of the monitor under the cursor and starts the frame timer. Must run on the overlay thread; does nothing when no bitmap can be made. The first frame is drawn with no closing time set, so it cannot ask to close.
unsafe fn open(hwnd: HWND, top: bool) {
    let _ = KillTimer(Some(hwnd), NOTICE_TIMER);
    let scale = scale_here();
    let h = (HEIGHT * scale).round() as i32;
    let Some(canvas) = Canvas::new((MAX_WIDTH * scale).round() as i32, h, scale) else { return drew(false, "no bitmap could be made") };
    let (centre_x, y) = anchor(h, scale, top);
    let mut overlay = Overlay {
        canvas,
        caption: Caption::new(scale, !animations_on()),
        palette: theme_palette(),
        centre_x,
        top: y,
        at_top: top,
        shown_at: std::time::Instant::now(),
        closing_since: None,
    };
    overlay.step(hwnd);
    let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
    let _ = SetWindowPos(hwnd, Some(HWND_TOPMOST), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
    STATE.with(|s| *s.borrow_mut() = Some(overlay));
    SetTimer(Some(hwnd), TIMER, FRAME_MS, None);
}

/// A notice is painted once; the window keeps its pixels, so no bitmap or timer is needed afterwards except the one that hides it. The previous pill must not stay up with no timer to end it. Never wider than the caption pill; a longer message ends in an ellipsis.
unsafe fn open_notice(hwnd: HWND, top: bool) {
    let _ = KillTimer(Some(hwnd), TIMER);
    STATE.with(|s| s.borrow_mut().take());
    let (scale, text, palette) = (scale_here(), notice_text(), theme_palette());
    let inter = Private::load();
    let font = notice::font(&inter, scale);
    let h = (HEIGHT * scale).round() as i32;
    let w = notice::width(&text, font, scale, (BASE_WIDTH * scale).round() as i32).min((MAX_WIDTH * scale).round() as i32);
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
    } else {
        drew(false, "no bitmap could be made for the message");
        let _ = ShowWindow(hwnd, SW_HIDE);
    }
    let _ = DeleteObject(font.into());
}

/// Gives the captions the pill is owed (those of the dictation that owns it now) to its caption. Must run on the overlay thread.
fn add_captions() {
    let queued = take_captions();
    STATE.with(|s| {
        if let Some(o) = s.borrow_mut().as_mut() {
            queued.iter().for_each(|(index, text)| o.caption.set(*index, text));
        }
    });
}

impl Overlay {
    /// Whether the live text is on show: there is some, and the recording has run long enough for it.
    fn text_visible(&self) -> bool {
        !self.caption.is_empty() && self.shown_at.elapsed() >= CAPTION_AFTER
    }

    /// The width the pill is heading for: its resting width, or as wide as the typed text needs up to `MAX_WIDTH`.
    unsafe fn target_width(&self, text: bool) -> f32 {
        let s = self.canvas.scale;
        let base = BASE_WIDTH * s;
        if !text {
            return base;
        }
        (2.0 * PAD_X * s + self.caption.visible_width() as f32 + caption::CARET_ROOM * s).clamp(base, MAX_WIDTH * s)
    }

    /// The rectangle the caption is drawn in for a pill `width` px wide.
    fn text_area(&self, width: i32) -> RECT {
        let (s, h) = (self.canvas.scale, self.canvas.h);
        RECT { left: (PAD_X * s) as i32, top: h / 5, right: width - (PAD_X * s) as i32, bottom: h - h / 5 }
    }

    /// One frame: type a little more, size the pill to the text, paint pill and bars (until text takes over), draw the caption, hand it over.
    /// Returns true when the dictation is over and the caption has finished (or waited long enough), so the caller closes the pill. Typing is the animation: the pill simply follows the text.
    unsafe fn step(&mut self, hwnd: HWND) -> bool {
        self.caption.advance();
        let text = self.text_visible();
        let w = self.target_width(text).round() as i32;
        let frame = Frame {
            width: w,
            level: crate::audio::level(),
            live: crate::audio::live(),
            bars_alpha: if text { 1.0 - self.caption.presence() } else { 1.0 },
            palette: &self.palette,
        };
        self.canvas.paint(&frame);
        if text {
            let area = self.text_area(w);
            if self.caption.draw(self.canvas.dc, &area, &self.palette, self.canvas.scale) {
                self.canvas.fade_left(&area, self.palette.card, (EDGE_FADE * self.canvas.scale) as i32);
            }
            self.canvas.opaque(&area);
        }
        blit(hwnd, &self.canvas, w, POINT { x: self.centre_x - w / 2, y: self.top });
        self.closing_since.is_some_and(|at| self.caption.typing_done() || at.elapsed() > LINGER)
    }
}
