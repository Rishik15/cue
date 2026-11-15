//! File: screen.rs
//! Purpose: Where the selected text is on the screen and how much room its monitor has, so a popup can open beside it.
//! Contents:
//! - caret_in — the caret of one window, as a screen rectangle; pointer_box — a line-high box round the pointer
//! - selection_anchor — the selection's rectangle and the work area (the screen minus the taskbar) and scale of its monitor, in physical pixels
//! - `selected` (UI Automation) first, else the text caret, else a box around the pointer (`around`)

use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::UI::WindowsAndMessaging::{GetAncestor, GetCursorPos, GetGUIThreadInfo, GA_ROOT, GUITHREADINFO};

use super::monitor::at_point;
use crate::platform::Rect;

/// The focused thread's GUI state, read once. Safety: `info` is a correctly sized out-parameter.
fn gui_info() -> Option<GUITHREADINFO> {
    let mut info = GUITHREADINFO { cbSize: std::mem::size_of::<GUITHREADINFO>() as u32, ..Default::default() };
    unsafe { GetGUIThreadInfo(0, &mut info) }.ok()?;
    Some(info)
}

/// The caret in one GUI-state snapshot (classic controls), as a screen rectangle. Input: the snapshot. Return: the rectangle, None when it has no caret window or the conversion failed.
fn caret_of(info: &GUITHREADINFO) -> Option<Rect> {
    if info.hwndCaret.is_invalid() {
        return None;
    }
    let (mut top_left, mut bottom_right) =
        (POINT { x: info.rcCaret.left, y: info.rcCaret.top }, POINT { x: info.rcCaret.right, y: info.rcCaret.bottom });
    unsafe {
        ClientToScreen(info.hwndCaret, &mut top_left).ok().ok()?;
        ClientToScreen(info.hwndCaret, &mut bottom_right).ok().ok()?
    };
    Some(Rect { left: top_left.x, top: top_left.y, right: bottom_right.x, bottom: bottom_right.y })
}

/// The text caret of the focused window, as a screen rectangle.
fn caret() -> Option<Rect> {
    caret_of(&gui_info()?)
}

/// The text caret when it is in `window` (or a child of it), else None: a caret of another window says nothing about the word the user selected. The ownership check and the rectangle come from the same snapshot, so the caret cannot move to another window between them. Input: a window handle (`foreground_window`). Return: the caret's screen rectangle.
pub fn caret_in(window: isize) -> Option<Rect> {
    let info = gui_info()?;
    let root = |w: HWND| unsafe { GetAncestor(w, GA_ROOT) };
    (!info.hwndCaret.is_invalid() && root(info.hwndCaret) == root(HWND(window as _))).then(|| caret_of(&info)).flatten()
}

/// A line-high box round the pointer (the last place to anchor a card when the app gives neither a selection nor a caret). Return: the box, or None when Windows cannot say.
pub fn pointer_box() -> Option<Rect> {
    pointer()
}

/// Where the pointer is, as a box about a line of text high: an app that gives no selection place (a browser page) leaves only the pointer, and a point would let the card sit on the word it is looking up. Safety: `at` is a correctly sized out-parameter.
fn pointer() -> Option<Rect> {
    let mut at = POINT::default();
    unsafe { GetCursorPos(&mut at) }.ok()?;
    Some(around(at.x, at.y, monitor_scale(at)))
}

/// DPI scale of the monitor under `at`, the one the card is drawn for (not the primary one's).
fn monitor_scale(at: POINT) -> f32 {
    at_point(at.x, at.y).map_or(1.0, |d| d.scale)
}

/// A selection rectangle is believed only when it could be a word or a line: some apps answer with the rectangle of a whole paragraph, page or editor, and a card placed by that would sit far from the word.
fn plausible(rect: &Rect) -> bool {
    let scale = monitor_scale(POINT { x: (rect.left + rect.right) / 2, y: (rect.top + rect.bottom) / 2 });
    fits_a_line((rect.right - rect.left) as f32 / scale, (rect.bottom - rect.top) as f32 / scale)
}

/// Sizes in dip: wide enough for a 64-character selection in large type, tall enough for a big heading.
fn fits_a_line(width: f32, height: f32) -> bool {
    width <= 1000.0 && height <= 220.0
}

/// A box in physical pixels around a point, one line of ordinary text (20 dip) high with the pointer in its middle: the pointer is usually on the word, so the card keeps the same gap from the word that it keeps from a rectangle an app reports.
fn around(x: i32, y: i32, scale: f32) -> Rect {
    let px = |dip: f32| (dip * scale).round() as i32;
    Rect { left: x - px(12.0), top: y - px(10.0), right: x + px(12.0), bottom: y + px(10.0) }
}

/// `selected` is the selection's rectangle from UI Automation when the app gave one; otherwise the text caret, then the pointer. Return: that rectangle, and the work area and DPI scale of the one monitor under its middle, or None when Windows cannot say.
pub fn selection_anchor(selected: Option<Rect>) -> Option<(Rect, Rect, f32)> {
    let target = selected.filter(plausible).or_else(caret).or_else(pointer)?;
    let display = at_point((target.left + target.right) / 2, (target.top + target.bottom) / 2)?;
    Some((target, display.work, display.scale))
}
