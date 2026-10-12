//! Purpose: Where the overlay window goes and how a painted canvas reaches the screen.
//! Contents: scale_here — DPI scale of the monitor under the cursor; anchor — top or bottom centre of the monitor under the cursor; blit — hand a canvas to the compositor.

use windows::Win32::Foundation::{COLORREF, HWND, POINT, SIZE};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::*;

use super::canvas::Canvas;

/// Distance from the work area edge. The bottom sits higher so the pill clears the taskbar and rounded screen corners.
const TOP_GAP: f32 = 28.0;
const BOTTOM_GAP: f32 = 64.0;
/// The monitor under the cursor, where the pill appears.
unsafe fn cursor_monitor() -> HMONITOR {
    let mut cursor = POINT::default();
    let _ = GetCursorPos(&mut cursor);
    MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST)
}
/// DPI scale of that monitor (the window itself may still sit on another one from its last showing).
pub(super) unsafe fn scale_here() -> f32 {
    let (mut dpi_x, mut dpi_y) = (96, 96);
    let _ = GetDpiForMonitor(cursor_monitor(), MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y);
    dpi_x.max(96) as f32 / 96.0
}

/// Centre and top edge of the pill: top or bottom centre of the work area of the monitor under the cursor.
pub(super) unsafe fn anchor(h: i32, scale: f32, top: bool) -> (i32, i32) {
    let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
    let _ = GetMonitorInfoW(cursor_monitor(), &mut mi);
    let y = if top { mi.rcWork.top + (TOP_GAP * scale) as i32 } else { mi.rcWork.bottom - h - (BOTTOM_GAP * scale) as i32 };
    ((mi.rcWork.left + mi.rcWork.right) / 2, y)
}

/// Hand the left `width` pixels of an already painted canvas to the compositor at `pos`.
pub(super) unsafe fn blit(hwnd: HWND, cv: &Canvas, width: i32, pos: POINT) {
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
