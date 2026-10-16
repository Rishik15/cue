//! Purpose: Where the pointer is and how much room its monitor has, so a popup can open beside the selected word.
//! Contents: cursor_anchor — the pointer and the work area (the screen minus the taskbar) of the monitor it is on, in physical pixels.

use windows::Win32::Foundation::POINT;
use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST};
use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

use crate::platform::{Point, Rect};

pub fn cursor_anchor() -> Option<(Point, Rect)> {
    let mut at = POINT::default();
    unsafe { GetCursorPos(&mut at) }.ok()?;
    let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
    unsafe { GetMonitorInfoW(MonitorFromPoint(at, MONITOR_DEFAULTTONEAREST), &mut info) }.ok().ok()?;
    let work = info.rcWork;
    Some((Point { x: at.x, y: at.y }, Rect { left: work.left, top: work.top, right: work.right, bottom: work.bottom }))
}
