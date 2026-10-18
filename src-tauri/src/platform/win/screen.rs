//! Purpose: Where the selected text is on the screen and how much room its monitor has, so a popup can open beside it.
//! Contents: selection_anchor — the selection's rectangle and the work area (the screen minus the taskbar) of its monitor, in physical pixels; `selected` (UI Automation) first, else the text caret, else a box around the pointer (`around`).

use windows::Win32::Foundation::{POINT, RECT};
use windows::Win32::Graphics::Gdi::{ClientToScreen, GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, GetGUIThreadInfo, GUITHREADINFO};

use crate::platform::Rect;

/// The text caret of the focused window (classic controls), as a screen rectangle.
fn caret() -> Option<Rect> {
    let mut info = GUITHREADINFO { cbSize: std::mem::size_of::<GUITHREADINFO>() as u32, ..Default::default() };
    unsafe { GetGUIThreadInfo(0, &mut info) }.ok()?;
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

/// Where the pointer is, as a box about a line of text high: an app that gives no selection place (a browser page) leaves only the pointer, and a point would let the card sit on the word it is looking up.
fn pointer() -> Option<Rect> {
    let mut at = POINT::default();
    unsafe { GetCursorPos(&mut at) }.ok()?;
    Some(around(at.x, at.y, monitor_scale(at)))
}

/// DPI scale of the monitor under `at`, the one the card is drawn for (not the primary one's).
fn monitor_scale(at: POINT) -> f32 {
    let (mut dpi_x, mut dpi_y) = (96, 96);
    // SAFETY: a plain query on the nearest monitor.
    let _ = unsafe { GetDpiForMonitor(MonitorFromPoint(at, MONITOR_DEFAULTTONEAREST), MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) };
    dpi_x.max(96) as f32 / 96.0
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

fn work_area(target: &Rect) -> Option<RECT> {
    let middle = POINT { x: (target.left + target.right) / 2, y: (target.top + target.bottom) / 2 };
    let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
    unsafe { GetMonitorInfoW(MonitorFromPoint(middle, MONITOR_DEFAULTTONEAREST), &mut info) }.ok().ok()?;
    Some(info.rcWork)
}

/// `selected` is the selection's rectangle from UI Automation when the app gave one; otherwise the text caret, then the pointer.
pub fn selection_anchor(selected: Option<Rect>) -> Option<(Rect, Rect)> {
    let target = selected.filter(plausible).or_else(caret).or_else(pointer)?;
    let work = work_area(&target)?;
    Some((target, Rect { left: work.left, top: work.top, right: work.right, bottom: work.bottom }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_whole_paragraph_or_editor_is_not_a_selection_rectangle() {
        assert!(fits_a_line(180.0, 28.0), "a word");
        assert!(fits_a_line(260.0, 110.0), "a word in a big heading");
        assert!(!fits_a_line(1400.0, 40.0), "a page-wide line");
        assert!(!fits_a_line(300.0, 700.0), "a whole editor");
    }

    #[test]
    fn the_pointer_box_leaves_room_below_and_scales() {
        let one = around(100, 100, 1.0);
        assert_eq!((one.left, one.right, one.top, one.bottom), (88, 112, 90, 110));
        assert_eq!(around(100, 100, 2.0).bottom - 100, 20);
    }
}
