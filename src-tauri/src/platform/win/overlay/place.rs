//! File: place.rs
//! Purpose: Where the overlay window goes and how a painted canvas reaches the screen. The monitor questions are answered by `win::monitor`, the same code every native surface uses.
//! Contents:
//! - scale_here — DPI scale of the monitor under the cursor
//! - anchor — top or bottom centre of that monitor's work area
//! - blit — hand a canvas to the compositor
//! - drew — report a drawing failure once
//! - animations_on — the system animation setting

use windows::core::BOOL;
use windows::Win32::Foundation::{COLORREF, HWND, POINT, SIZE};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use super::super::monitor::{at_point, under_cursor, Display};
use std::sync::atomic::{AtomicBool, Ordering};

use super::canvas::Canvas;

/// Distance from the work area edge. The bottom sits higher so the pill clears the taskbar and rounded screen corners.
const TOP_GAP: f32 = 28.0;
const BOTTOM_GAP: f32 = 64.0;

/// The monitor under the cursor, or the one nearest the screen origin when the pointer cannot be read (as in 0.3.5), or a 96 dpi 1280 by 720 display at the origin when Windows names no monitor at all.
fn here() -> Display {
    under_cursor()
        .or_else(|| at_point(0, 0))
        .unwrap_or(Display { work: crate::platform::Rect { left: 0, top: 0, right: 1280, bottom: 720 }, scale: 1.0 })
}

/// DPI scale of the monitor under the cursor (the window itself may still sit on another one from its last showing).
pub(super) fn scale_here() -> f32 {
    here().scale
}

/// Centre and top edge of the pill: top or bottom centre of the work area of the monitor under the cursor. Input: the pill's height in px, its scale and whether it sits at the top.
pub(super) fn anchor(h: i32, scale: f32, top: bool) -> (i32, i32) {
    let work = here().work;
    let y = if top { work.top + (TOP_GAP * scale) as i32 } else { work.bottom - h - (BOTTOM_GAP * scale) as i32 };
    ((work.left + work.right) / 2, y)
}

/// Whether the system lets windows animate (Settings, Accessibility, Visual effects, Animation effects). Read afresh each time the pill opens. Return: true when it cannot be read.
///
/// Safety: `on` is a valid BOOL out-parameter of the size the query writes.
pub(super) fn animations_on() -> bool {
    let mut on = BOOL(1);
    let read =
        unsafe { SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, Some((&raw mut on).cast()), SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0)) };
    read.is_err() || on.as_bool()
}

/// Whether the last drawing attempt failed; the user is told once per run of failures, because telling them goes through this same pill.
static FAILING: AtomicBool = AtomicBool::new(false);

/// Records the outcome of a drawing step. Input: whether it worked and, for a failure, what could not be done. A failure is reported to the user once until a step works again.
pub(super) fn drew(worked: bool, what: &str) {
    if worked {
        FAILING.store(false, Ordering::Relaxed);
    } else if !FAILING.swap(true, Ordering::Relaxed) {
        crate::log::note(&format!("pill: {what}"));
        crate::platform::report(&format!("The recording pill could not be drawn: {what}."));
    }
}

/// Hand the left `width` pixels of an already painted canvas to the compositor at `pos`.
///
/// `hwnd` must be the overlay window and `cv` a live canvas; both are used on the overlay thread only.
pub(super) unsafe fn blit(hwnd: HWND, cv: &Canvas, width: i32, pos: POINT) {
    let blend = BLENDFUNCTION { BlendOp: AC_SRC_OVER as u8, BlendFlags: 0, SourceConstantAlpha: 255, AlphaFormat: AC_SRC_ALPHA as u8 };
    let size = SIZE { cx: width, cy: cv.h };
    let result = UpdateLayeredWindow(
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
    drew(result.is_ok(), "Windows refused to update its window");
}
