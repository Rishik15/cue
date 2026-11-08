//! File: place.rs
//! Purpose: Where a floating card goes and which way its arrow points: under the selected text when there is room, else over it, else beside it, kept inside the work area of its monitor.
//! Contents:
//! - Side — the card edge the arrow sits on
//! - Placed — the card's top-left corner, the side and the arrow's distance along that edge
//! - place — choose the first side that fits
//! - fit_logical — shrink a window's size to the work area

use super::Rect;

/// Space kept between the popup and the screen edge.
const EDGE: i32 = 8;

/// The card edge the arrow sits on (it points away from the card, at the selection).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Side {
    Top,
    Bottom,
    Left,
    Right,
}

/// Where a card goes: its top-left corner, the side its arrow is on and the arrow's place along that side.
#[derive(Debug, PartialEq)]
pub struct Placed {
    pub x: i32,
    pub y: i32,
    pub side: Side,
    /// Distance of the arrow's middle from the start of its edge (left for top and bottom, top for left and right).
    pub offset: i32,
}

/// Chooses the first side that fits (under, over, right, left) and keeps the card inside `work`. Input: the selection `target`, the work area, the card size, `reach` and `inset`. Return: the placement. `reach` is the space the arrow and its gap take between the selection and the card; `inset` keeps the arrow clear of the card's rounded corners.
pub fn place(target: Rect, work: Rect, card: (i32, i32), reach: i32, inset: i32) -> Placed {
    let (w, h) = card;
    let (mid_x, mid_y) = ((target.left + target.right) / 2, (target.top + target.bottom) / 2);
    let across = |size: i32, lo: i32, hi: i32, mid: i32| (mid - size / 2).clamp(lo + EDGE, (hi - size - EDGE).max(lo + EDGE));
    let (x_centred, y_centred) = (across(w, work.left, work.right, mid_x), across(h, work.top, work.bottom, mid_y));
    let along = |start: i32, size: i32, mid: i32| (mid - start).clamp(inset, (size - inset).max(inset));
    let below = Placed { x: x_centred, y: target.bottom + reach, side: Side::Top, offset: along(x_centred, w, mid_x) };
    let above = Placed { x: x_centred, y: target.top - reach - h, side: Side::Bottom, offset: along(x_centred, w, mid_x) };
    let right = Placed { x: target.right + reach, y: y_centred, side: Side::Left, offset: along(y_centred, h, mid_y) };
    let left = Placed { x: target.left - reach - w, y: y_centred, side: Side::Right, offset: along(y_centred, h, mid_y) };
    let fits =
        |p: &Placed| p.x >= work.left + EDGE && p.y >= work.top + EDGE && p.x + w + EDGE <= work.right && p.y + h + EDGE <= work.bottom;
    let (fallback_x, fallback_y) = (below.x, below.y.clamp(work.top + EDGE, (work.bottom - h - EDGE).max(work.top + EDGE)));
    [below, above, right, left].into_iter().find(fits).unwrap_or(Placed {
        x: fallback_x,
        y: fallback_y,
        side: Side::Top,
        offset: along(fallback_x, w, mid_x),
    })
}

/// Shrinks a window's logical size so it fits a work area with `margin` logical pixels to spare on every side. Input: the wanted size, the work area in physical pixels, the monitor's scale and the margin. Return: the size, never larger than wanted and never below 1.
pub fn fit_logical(size: (f64, f64), area: Rect, scale: f64, margin: f64) -> (f64, f64) {
    let room = |lo: i32, hi: i32| (f64::from(hi - lo) / scale - 2.0 * margin).max(1.0);
    (size.0.min(room(area.left, area.right)), size.1.min(room(area.top, area.bottom)))
}
