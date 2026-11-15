//! File: place.rs
//! Purpose: Sizing a webview window to the work area of its monitor.
//! Contents: fit_logical — shrink a window's size to the work area.

use super::Rect;

/// Shrinks a window's logical size so it fits a work area with `margin` logical pixels to spare on every side. Input: the wanted size, the work area in physical pixels, the monitor's scale and the margin. Return: the size, never larger than wanted and never below 1.
pub fn fit_logical(size: (f64, f64), area: Rect, scale: f64, margin: f64) -> (f64, f64) {
    let room = |lo: i32, hi: i32| (f64::from(hi - lo) / scale - 2.0 * margin).max(1.0);
    (size.0.min(room(area.left, area.right)), size.1.min(room(area.top, area.bottom)))
}
