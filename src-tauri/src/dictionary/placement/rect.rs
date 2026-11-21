//! File: dictionary/placement/rect.rs
//! Purpose: Rectangle arithmetic for placement that cannot overflow: sizes and overlaps in 64 bits, saturating narrowing, the usable area inside a work area's edge margin.
//! Contents: grown — a rectangle with a border added; sane / narrow — a safe scale and a saturating i32; width / height / solid / intersect / shared — measures of rectangles; inner — the work area less its margin; fitted — a size cut to an area.

use crate::platform::Rect;

/// The margin kept from the work area's edge, in dip.
const EDGE: f64 = 8.0;

/// A rectangle with `by` added on every side (the window round a card), saturating.
pub fn grown(r: Rect, by: i32) -> Rect {
    let by = i64::from(by);
    let add = |v: i32, d: i64| narrow(i64::from(v) + d);
    Rect { left: add(r.left, -by), top: add(r.top, -by), right: add(r.right, by), bottom: add(r.bottom, by) }
}

/// A scale Windows could report: finite and between half and eight times; anything else counts as 1.
pub(super) fn sane(scale: f64) -> f64 {
    if scale.is_finite() && (0.5..=8.0).contains(&scale) {
        scale
    } else {
        1.0
    }
}

/// `v` as an i32, saturating.
pub(super) fn narrow(v: i64) -> i32 {
    v.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

/// Width of a rectangle, in 64 bits so extreme coordinates cannot overflow.
pub(super) fn width(r: &Rect) -> i64 {
    i64::from(r.right) - i64::from(r.left)
}

/// Height of a rectangle, in 64 bits.
pub(super) fn height(r: &Rect) -> i64 {
    i64::from(r.bottom) - i64::from(r.top)
}

/// Whether the rectangle has a positive size.
pub(super) fn solid(r: &Rect) -> bool {
    width(r) > 0 && height(r) > 0
}

/// The part two rectangles share, if any.
pub(super) fn intersect(a: &Rect, b: &Rect) -> Option<Rect> {
    let r = Rect { left: a.left.max(b.left), top: a.top.max(b.top), right: a.right.min(b.right), bottom: a.bottom.min(b.bottom) };
    solid(&r).then_some(r)
}

/// The area two rectangles share.
pub(super) fn shared(a: &Rect, b: &Rect) -> i64 {
    intersect(a, b).map_or(0, |r| width(&r) * height(&r))
}

/// The work area with the edge margin taken off every side (nothing is taken off an area too small for it). None for an empty work area.
pub(super) fn inner(work: Rect, scale: f64) -> Option<Rect> {
    if !solid(&work) {
        return None;
    }
    let edge = (EDGE * scale).round() as i64;
    let edge = if width(&work) > 2 * edge && height(&work) > 2 * edge { edge } else { 0 };
    let moved = |v: i32, by: i64| narrow(i64::from(v) + by);
    Some(Rect {
        left: moved(work.left, edge),
        top: moved(work.top, edge),
        right: moved(work.right, -edge),
        bottom: moved(work.bottom, -edge),
    })
}

/// A size no larger than the area, at least 1 by 1.
pub(super) fn fitted(size: (i32, i32), area: &Rect) -> (i64, i64) {
    (i64::from(size.0).clamp(1, width(area)), i64::from(size.1).clamp(1, height(area)))
}
