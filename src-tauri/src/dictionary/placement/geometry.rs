//! File: dictionary/placement/geometry.rs
//! Purpose: Choosing a side and a position for a card beside its lines, centring a window with no anchor, resizing a placed card and locating its arrow.
//! Contents: GAP_SELECTION / GAP_POINT — the gap in dip; Candidate / ends / ideal / candidate / flipped — the scoring of the four sides; place — the best side; centred — the window with no anchor; resize — a new size that keeps the edge next to the selection fixed; `Placed::arrow` — where the arrow sits on the card's edge.

use super::rect::{fitted, height, inner, narrow, sane, shared, solid, width};
use super::{Kind, Placed, Request, Side, ARROW_OFFSET};
use crate::platform::Rect;

/// The gap between the selection and the card in dip, with the arrow in it: the native card's 8 dip arrow and the 3 dip left between its tip and the text.
const GAP_SELECTION: f64 = 11.0;
/// The gap in dip when the anchor is only a caret or the pointer.
const GAP_POINT: f64 = 4.0;

/// One candidate: its side, ideal top-left, the card moved into the area, and how it scores (larger is better).
struct Candidate {
    side: Side,
    rect: Rect,
    target: i64,
    score: (bool, bool, i64, i64, i64),
}

/// The first line (top, then left) and the last line (greatest bottom, then left) of a selection.
fn ends(lines: &[Rect]) -> Option<(Rect, Rect)> {
    let first = lines.iter().min_by_key(|l| (l.top, l.left))?;
    let last = lines.iter().max_by_key(|l| (l.bottom, l.left))?;
    Some((*first, *last))
}

/// The ideal top-left of the card on `side`: below the last line, above the first, or beside the first. A selection's card (`offset` is the arrow's place in px) puts its arrow on the line's middle at that offset from its start edge (left, or right when right to left, for below and above; top beside), so it extends away from the arrow; a caret's or the pointer's card (no arrow) has its left edge on the line's start (its right edge on the line's end when right to left) and its top on the line's top. Input: the side, the first and last lines, the card's size, the gap, the arrow's offset (None without an arrow) and the direction. Return: the corner and the arrow's coordinate on the selection.
fn ideal(side: Side, (first, last): (Rect, Rect), size: (i64, i64), gap: i64, offset: Option<i64>, rtl: bool) -> (i64, i64, i64) {
    let line = if side == Side::Below { last } else { first };
    let centre_x = (i64::from(line.left) + i64::from(line.right)) / 2;
    let centre_y = (i64::from(line.top) + i64::from(line.bottom)) / 2;
    let across = match (offset, rtl) {
        (Some(off), true) => centre_x + off - size.0,
        (Some(off), false) => centre_x - off,
        (None, true) => i64::from(line.right) - size.0,
        (None, false) => i64::from(line.left),
    };
    let down = offset.map_or(i64::from(line.top), |off| centre_y - off);
    match side {
        Side::Below => (across, i64::from(line.bottom) + gap, centre_x),
        Side::Above => (across, i64::from(line.top) - gap - size.1, centre_x),
        Side::Right => (i64::from(line.right) + gap, down, centre_y),
        _ => (i64::from(line.left) - gap - size.0, down, centre_y),
    }
}

/// Scores one side: moves the ideal corner into `area` and compares. Only a move toward or away from the selection counts against it; sliding along the selection (a card near the screen's edge, whose arrow then slides along its edge) is free, as on the native card.
fn candidate(order: i64, side: Side, req: &Request, area: &Rect, ends: (Rect, Rect), size: (i64, i64)) -> Candidate {
    let selection = req.kind == Kind::Selection;
    let scale = sane(req.scale);
    let gap = (if selection { GAP_SELECTION } else { GAP_POINT } * scale).round() as i64;
    let offset = selection.then(|| (ARROW_OFFSET * scale).round() as i64);
    let (x, y, target) = ideal(side, ends, size, gap, offset, req.rtl);
    let left = x.clamp(i64::from(area.left), i64::from(area.right) - size.0);
    let top = y.clamp(i64::from(area.top), i64::from(area.bottom) - size.1);
    let rect = Rect { left: narrow(left), top: narrow(top), right: narrow(left + size.0), bottom: narrow(top + size.1) };
    let pushed = if matches!(side, Side::Below | Side::Above) { top - y } else { left - x };
    let covers = req.lines.iter().any(|l| shared(l, &rect) > 0);
    Candidate { side, rect, target, score: (pushed == 0, !covers, shared(&rect, area), -pushed.abs(), -order) }
}

/// Whether a window anchored at the pointer extends left of it: a pointer's window starts at the pointer and runs right, unless the work area has no room for it there, when its right edge sits at the pointer instead (as a right-to-left caret's card does). Input: the request, the first line, the usable area and the window's size. Return: the direction to align with.
fn flipped(req: &Request, first: &Rect, area: &Rect, size: (i64, i64)) -> bool {
    req.rtl || (req.kind == Kind::Cursor && i64::from(first.left) + size.0 > i64::from(area.right))
}

/// Places a card beside a selection, or a window beside the pointer (the search window: below and to the right of it, above or to its left when there is no room). Input: the request. Return: None when the work area is empty or no line is valid; otherwise the best of the four sides, always inside the work area, never larger than it.
pub fn place(req: &Request) -> Option<Placed> {
    let area = inner(req.work, sane(req.scale))?;
    let valid: Vec<Rect> = req.lines.iter().copied().filter(solid).collect();
    let ends = ends(&valid)?;
    let size = fitted(req.size, &area);
    let req = Request { lines: &valid, rtl: flipped(req, &ends.0, &area, size), ..*req };
    let best = [Side::Below, Side::Above, Side::Right, Side::Left]
        .into_iter()
        .enumerate()
        .map(|(order, side)| candidate(order as i64, side, &req, &area, ends, size))
        .max_by_key(|c| c.score)?;
    Some(Placed { rect: best.rect, side: best.side, target: (req.kind == Kind::Selection).then(|| narrow(best.target)), rtl: req.rtl })
}

/// Places a window with no anchor (the search window when the pointer could not be read) in the middle of the work area. Input: the work area, the wanted size and the scale. Return: None for an empty work area; the window is shrunk to the area when it is larger.
pub fn centred(work: Rect, size: (i32, i32), scale: f64) -> Option<Placed> {
    let area = inner(work, sane(scale))?;
    let (w, h) = fitted(size, &area);
    let left = i64::from(area.left) + (width(&area) - w) / 2;
    let top = i64::from(area.top) + (height(&area) - h) / 2;
    let rect = Rect { left: narrow(left), top: narrow(top), right: narrow(left + w), bottom: narrow(top + h) };
    Some(Placed { rect, side: Side::Centre, target: None, rtl: false })
}

/// A card of a new size: the edge next to the selection stays where it is (below keeps its top, above its bottom, right its left, left its right), so the card grows away from the text; a size the room on that side cannot hold is cut to the room (its page scrolls). The other axis keeps its start edge (its end edge right to left) and moves only to stay in the work area. Input: the card as placed, the new size, the work area and the scale. Return: None for an empty work area.
pub fn resize(prev: &Placed, size: (i32, i32), work: Rect, scale: f64) -> Option<Placed> {
    let area = inner(work, sane(scale))?;
    let (mut w, mut h) = fitted(size, &area);
    let (r, side) = (&prev.rect, prev.side);
    let (near_left, near_top) = (i64::from(r.left), i64::from(r.top));
    match side {
        Side::Below | Side::Centre => h = h.min(i64::from(area.bottom) - near_top.max(i64::from(area.top))).max(1),
        Side::Above => h = h.min(i64::from(r.bottom) - i64::from(area.top)).max(1),
        Side::Right => w = w.min(i64::from(area.right) - near_left.max(i64::from(area.left))).max(1),
        Side::Left => w = w.min(i64::from(r.right) - i64::from(area.left)).max(1),
    }
    let along_end = prev.rtl && matches!(side, Side::Below | Side::Above);
    let mut left = match side {
        Side::Left => i64::from(r.right) - w,
        _ if along_end => i64::from(r.right) - w,
        _ => near_left,
    };
    let mut top = if side == Side::Above { i64::from(r.bottom) - h } else { near_top };
    left = left.clamp(i64::from(area.left), (i64::from(area.right) - w).max(i64::from(area.left)));
    top = top.clamp(i64::from(area.top), (i64::from(area.bottom) - h).max(i64::from(area.top)));
    let rect = Rect { left: narrow(left), top: narrow(top), right: narrow(left + w), bottom: narrow(top + h) };
    Some(Placed { rect, ..*prev })
}

impl Placed {
    /// Where the arrow's middle sits on the card's edge next to the selection, in physical pixels from the edge's start (left for below and above, top for right and left): at the selection's middle (`ARROW_OFFSET` from the start edge unless a screen edge pushed the card, when it slides along the edge), kept `inset` from the corners so it clears their rounding. None for a card with no arrow (a caret, the pointer, no selection).
    pub fn arrow(&self, inset: i32) -> Option<i32> {
        let target = i64::from(self.target?);
        let (start, len) = match self.side {
            Side::Below | Side::Above => (i64::from(self.rect.left), width(&self.rect)),
            Side::Right | Side::Left => (i64::from(self.rect.top), height(&self.rect)),
            Side::Centre => return None,
        };
        let inset = i64::from(inset).min(len / 2);
        Some(narrow((target - start).clamp(inset, len - inset)))
    }
}
