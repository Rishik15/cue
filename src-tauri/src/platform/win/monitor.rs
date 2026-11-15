//! File: monitor.rs
//! Purpose: The one place that answers "which screen is this on": the work area (the screen minus the taskbar, physical pixels) and the DPI scale of the monitor under a point, a window or the pointer. The pill, the card, the popup menu and the selection anchor all place themselves from it.
//! Contents: Display — a monitor's work area and scale; at_point / under_cursor / of_window — find one; all / displays — every monitor (for callers outside the platform, as (work area, scale) pairs); work_area_of — the same as a (Rect, scale) pair for one.

use windows::core::BOOL;
use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, MonitorFromPoint, MonitorFromWindow, HDC, HMONITOR, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

use crate::platform::Rect;

/// A monitor as a window needs it: where it may draw and how big a dip is there.
#[derive(Clone, Copy)]
pub struct Display {
    pub work: Rect,
    pub scale: f32,
}

/// Reads the work area and DPI scale of `monitor`. Return: None when Windows cannot say. Safety: As above; a failure leaves the 96 dpi default. Safety: Plain queries on a handle Windows returned; `info` is a correctly sized out-parameter.
fn describe(monitor: HMONITOR) -> Option<Display> {
    let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
    if !unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
        return None;
    }
    let (mut dpi_x, mut dpi_y) = (96, 96);
    let _ = unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) };
    let work = info.rcWork;
    Some(Display {
        work: Rect { left: work.left, top: work.top, right: work.right, bottom: work.bottom },
        scale: dpi_x.max(96) as f32 / 96.0,
    })
}

/// The monitor nearest the screen point `(x, y)`. Safety: A plain query; the nearest monitor is always found.
pub fn at_point(x: i32, y: i32) -> Option<Display> {
    describe(unsafe { MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST) })
}

/// The monitor under the pointer, where a pill or a menu appears. Safety: `at` is a correctly sized out-parameter.
pub fn under_cursor() -> Option<Display> {
    let mut at = POINT::default();
    unsafe { GetCursorPos(&mut at) }.ok()?;
    at_point(at.x, at.y)
}

/// The monitor a window is on. A stale handle makes Windows answer with the nearest monitor. Safety: A plain query on a window handle.
pub fn of_window(handle: isize) -> Option<Display> {
    describe(unsafe { MonitorFromWindow(HWND(handle as _), MONITOR_DEFAULTTONEAREST) })
}

/// `handle` is a window handle (`foreground_window`), or 0 for "where the pointer is". None when Windows cannot say.
pub fn work_area_of(handle: isize) -> Option<(Rect, f32)> {
    let display = if handle != 0 { of_window(handle) } else { under_cursor() }?;
    Some((display.work, display.scale))
}

/// Adds one monitor to the list `out` points at. Safety: `out` is the `Vec<Display>` `all` passed to `EnumDisplayMonitors`, alive for the whole call.
unsafe extern "system" fn collect(monitor: HMONITOR, _dc: HDC, _rect: *mut RECT, out: LPARAM) -> BOOL {
    if let Some(display) = describe(monitor) {
        (*(out.0 as *mut Vec<Display>)).push(display);
    }
    BOOL(1)
}

/// Every monitor, in the order Windows lists them. Safety: the list outlives the enumeration, which runs on this thread.
pub fn all() -> Vec<Display> {
    let mut list: Vec<Display> = Vec::new();
    let _ = unsafe { EnumDisplayMonitors(None, None, Some(collect), LPARAM(&mut list as *mut Vec<Display> as isize)) };
    list
}

/// Every monitor as its work area and DPI scale.
pub fn displays() -> Vec<(Rect, f32)> {
    all().into_iter().map(|d| (d.work, d.scale)).collect()
}
