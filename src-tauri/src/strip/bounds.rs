//! File: strip/bounds.rs
//! Purpose: The strip window's geometry crossing the page boundary: the selection is handed to the page in dip relative to its monitor's work area, and the window rectangle the page asks for comes back the same way, checked and turned into physical pixels kept on that work area. Pure, so it is tested without the OS.
//! Contents: Dip — a rectangle in dip; MAX_SIDE — the largest window side accepted; MAX_OFFSET — the farthest corner accepted; to_dip — a physical rectangle as seen from the work area; to_physical — a requested window rectangle, checked and clamped.

use serde::{Deserialize, Serialize};

use crate::platform::Rect;

/// The largest window width or height, in dip, the page may ask for: the list and the result card are far smaller, so anything bigger is a bug or a hostile page.
pub const MAX_SIDE: f64 = 1600.0;
/// The farthest a window corner may be from the work area's top-left, in dip: past any real screen, small enough that the physical sums cannot overflow.
pub const MAX_OFFSET: f64 = MAX_SIDE * 16.0;

/// A rectangle in dip (CSS pixels at the monitor's scale), relative to the top-left of the work area.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Dip {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// `rect` as the page sees it. Input: a physical rectangle, the work area it is measured from and the monitor scale. Return: the same rectangle in dip relative to the work area's top-left.
pub fn to_dip(rect: Rect, work: Rect, scale: f32) -> Dip {
    let s = f64::from(scale.max(0.5));
    Dip {
        x: f64::from(rect.left - work.left) / s,
        y: f64::from(rect.top - work.top) / s,
        w: f64::from(rect.right - rect.left) / s,
        h: f64::from(rect.bottom - rect.top) / s,
    }
}

/// The window rectangle the page asked for, in physical pixels. Input: the request, the work area and the monitor scale. Return: None for a request that is not finite, whose corner is more than `MAX_OFFSET` away, or whose size is not between 1 and `MAX_SIDE`; otherwise the rectangle, moved (never resized) so its middle stays on the work area, since the window's transparent margin may hang over the edge but the card may not leave the screen.
pub fn to_physical(dip: Dip, work: Rect, scale: f32) -> Option<Rect> {
    let sane =
        [dip.x, dip.y].iter().all(|v| v.abs() <= MAX_OFFSET) && (1.0..=MAX_SIDE).contains(&dip.w) && (1.0..=MAX_SIDE).contains(&dip.h);
    if !sane {
        return None;
    }
    let s = f64::from(scale.max(0.5));
    let (w, h) = ((dip.w * s).round() as i32, (dip.h * s).round() as i32);
    let keep = |start: f64, origin: i32, lo: i32, hi: i32, size: i32| {
        let at = origin + (start * s).round() as i32;
        at.clamp(lo - size / 2, (hi - size / 2).max(lo - size / 2))
    };
    let left = keep(dip.x, work.left, work.left, work.right, w);
    let top = keep(dip.y, work.top, work.top, work.bottom, h);
    Some(Rect { left, top, right: left + w, bottom: top + h })
}
