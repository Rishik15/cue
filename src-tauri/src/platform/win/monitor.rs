//! Purpose: Which monitor a window (or the pointer) is on, so a popup can open where the user is looking instead of always on the primary monitor.
//! Contents: work_area_of — the work area (the screen minus the taskbar, physical pixels) and the DPI scale of the monitor a window is on, or of the pointer's monitor when there is no window.

use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromPoint, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

use crate::platform::Rect;

/// `handle` is a window handle (`foreground_window`), or 0 for "where the pointer is". None when Windows cannot say.
pub fn work_area_of(handle: isize) -> Option<(Rect, f32)> {
    // SAFETY: plain queries; a stale window handle makes Windows answer with the nearest monitor.
    unsafe {
        let monitor = if handle != 0 {
            MonitorFromWindow(HWND(handle as _), MONITOR_DEFAULTTONEAREST)
        } else {
            let mut at = POINT::default();
            GetCursorPos(&mut at).ok()?;
            MonitorFromPoint(at, MONITOR_DEFAULTTONEAREST)
        };
        let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        if !GetMonitorInfoW(monitor, &mut info).as_bool() {
            return None;
        }
        let (mut dpi_x, mut dpi_y) = (96, 96);
        let _ = GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y);
        let work = info.rcWork;
        Some((Rect { left: work.left, top: work.top, right: work.right, bottom: work.bottom }, dpi_x.max(96) as f32 / 96.0))
    }
}
