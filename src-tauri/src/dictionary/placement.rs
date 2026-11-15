//! File: dictionary/placement.rs
//! Purpose: Where the dictionary card goes beside a selection, as pure geometry in physical virtual-screen pixels: no Tauri, no Windows. Negative origins, any monitor scale and hostile numbers are handled here, so the window code only converts once at its boundary and sets the result.
//! Contents: ARROW_INSET — how far the arrow keeps from a corner; grown — a rectangle with a border added; Side / Kind / Request / Placed — the vocabulary; choose_display — the monitor a selection is on; clip_lines — the selection's lines cut to a work area; place — choose a side (below, above, right, left) and a position; centred — the search window's place; resize — a new size that keeps the edge next to the selection fixed; is_rtl — whether a text reads right to left; `Placed::arrow` — where the arrow sits on the card's edge.
//! Candidates are scored in order: the card keeps its gap from the selection without being pushed (fits), it covers no selection line, more of it is on screen, it moves less toward or away from the selection, then the fixed order below, above, right, left. A window larger than the work area shrinks to it and its page scrolls.

use crate::platform::Rect;

/// Which side of the selection the card is on; `Centre` is the search window, which has no selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Below,
    Above,
    Right,
    Left,
    Centre,
}

/// What the lines are: a selection (a gap and an arrow pointing at it) or only a caret or the pointer (a tighter gap, no arrow: they may not mark the word).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Selection,
    Caret,
    Cursor,
}

/// What `place` needs: the selection's line rectangles (never their union), what they are, the work area of their monitor, the card's wanted size, the monitor's scale and the reading direction.
pub struct Request<'a> {
    pub lines: &'a [Rect],
    pub kind: Kind,
    pub work: Rect,
    pub size: (i32, i32),
    pub scale: f64,
    pub rtl: bool,
}

/// Where a card is: its rectangle, its side, the coordinate (x for below and above, y for right and left) of the selection point its arrow points at, and the reading direction its alignment used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placed {
    pub rect: Rect,
    pub side: Side,
    pub target: Option<i32>,
    pub rtl: bool,
}

/// The gap between the selection and the card in dip, with the arrow in it: the native card's 8 dip arrow and the 3 dip left between its tip and the text.
const GAP_SELECTION: f64 = 11.0;
/// How far the arrow's middle keeps from a card's corners, in dip: the 14 dip corner radius and the 14 dip half width of the arrow with its shoulders, so the arrow always stands on the straight edge.
pub const ARROW_INSET: f64 = 28.0;
/// The gap in dip when the anchor is only a caret or the pointer.
const GAP_POINT: f64 = 4.0;
/// The margin kept from the work area's edge, in dip.
const EDGE: f64 = 8.0;

/// The longest side the page may ask for, in dip: the search window is 360 by 480 and a card is 300 wide, so more than this is a card taller than any screen, which is cut to the work area and scrolls.
const MAX_SIDE: f64 = 1600.0;

/// `dip` as physical pixels at `scale`, for a size a page asks for: a side longer than `MAX_SIDE` is cut to it (placement then shrinks it to the work area), a value that is not a finite length of at least 1 dip is refused. Input: the dip length and the scale. Return: the pixels, or None.
pub fn to_px(dip: f64, scale: f64) -> Option<i32> {
    (dip.is_finite() && dip >= 1.0).then(|| (dip.min(MAX_SIDE) * scale).round() as i32)
}

/// Where a card points when the app gave no selection lines: the caret when it has an area, else a box round the pointer. A caret with no width or height is not a place (clipping to the work area would drop it and nothing could be placed). Input: the selection's lines, the caret of the word's window and the pointer's box. Return: the lines and what they are, None when nothing can be placed.
pub fn anchor(lines: &[Rect], caret: Option<Rect>, pointer: Option<Rect>) -> Option<(Vec<Rect>, Kind)> {
    if !lines.is_empty() {
        return Some((lines.to_vec(), Kind::Selection));
    }
    match caret.filter(|c| c.right > c.left && c.bottom > c.top) {
        Some(caret) => Some((vec![caret], Kind::Caret)),
        None => pointer.map(|p| (vec![p], Kind::Cursor)),
    }
}

/// A rectangle with `by` added on every side (the window round a card), saturating.
pub fn grown(r: Rect, by: i32) -> Rect {
    let by = i64::from(by);
    let add = |v: i32, d: i64| narrow(i64::from(v) + d);
    Rect { left: add(r.left, -by), top: add(r.top, -by), right: add(r.right, by), bottom: add(r.bottom, by) }
}

/// A scale Windows could report: finite and between half and eight times; anything else counts as 1.
fn sane(scale: f64) -> f64 {
    if scale.is_finite() && (0.5..=8.0).contains(&scale) {
        scale
    } else {
        1.0
    }
}

/// `v` as an i32, saturating.
fn narrow(v: i64) -> i32 {
    v.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

/// Width of a rectangle, in 64 bits so extreme coordinates cannot overflow.
fn width(r: &Rect) -> i64 {
    i64::from(r.right) - i64::from(r.left)
}

/// Height of a rectangle, in 64 bits.
fn height(r: &Rect) -> i64 {
    i64::from(r.bottom) - i64::from(r.top)
}

/// Whether the rectangle has a positive size.
fn solid(r: &Rect) -> bool {
    width(r) > 0 && height(r) > 0
}

/// The part two rectangles share, if any.
fn intersect(a: &Rect, b: &Rect) -> Option<Rect> {
    let r = Rect { left: a.left.max(b.left), top: a.top.max(b.top), right: a.right.min(b.right), bottom: a.bottom.min(b.bottom) };
    solid(&r).then_some(r)
}

/// The area two rectangles share.
fn shared(a: &Rect, b: &Rect) -> i64 {
    intersect(a, b).map_or(0, |r| width(&r) * height(&r))
}

/// The work area with the edge margin taken off every side (nothing is taken off an area too small for it). None for an empty work area.
fn inner(work: Rect, scale: f64) -> Option<Rect> {
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
fn fitted(size: (i32, i32), area: &Rect) -> (i64, i64) {
    (i64::from(size.0).clamp(1, width(area)), i64::from(size.1).clamp(1, height(area)))
}

/// The monitor a selection is on. Input: the selection's lines and each monitor's work area. Return: the index of the work area the lines overlap most, or, when none overlaps, the one nearest the lines; None without a work area or a line.
pub fn choose_display(lines: &[Rect], works: &[Rect]) -> Option<usize> {
    let overlap = |work: &Rect| lines.iter().map(|l| shared(l, work)).sum::<i64>();
    let best = works.iter().enumerate().filter(|(_, w)| solid(w)).max_by_key(|(i, w)| (overlap(w), std::cmp::Reverse(*i)))?;
    if overlap(best.1) > 0 {
        return Some(best.0);
    }
    let distance = |work: &Rect| lines.iter().map(|l| gap_between(l, work)).min().unwrap_or(i64::MAX);
    works.iter().enumerate().filter(|(_, w)| solid(w)).min_by_key(|(i, w)| (distance(w), *i)).map(|(i, _)| i)
}

/// The distance between two rectangles that do not overlap (the larger of the horizontal and vertical gaps; 0 when they overlap).
fn gap_between(a: &Rect, b: &Rect) -> i64 {
    let across = (i64::from(a.left) - i64::from(b.right)).max(i64::from(b.left) - i64::from(a.right)).max(0);
    let down = (i64::from(a.top) - i64::from(b.bottom)).max(i64::from(b.top) - i64::from(a.bottom)).max(0);
    across.max(down)
}

/// The selection's lines cut to a work area, empty and invalid lines dropped. A selection that is wholly outside the work area (its monitor was only the nearest) becomes its first line moved onto the nearest edge, so there is always something to anchor on. Input: the lines and the work area. Return: the visible lines; empty only for an empty work area or no valid line.
pub fn clip_lines(lines: &[Rect], work: Rect) -> Vec<Rect> {
    let clipped: Vec<Rect> = lines.iter().filter_map(|l| intersect(l, &work)).collect();
    if !clipped.is_empty() || !solid(&work) {
        return clipped;
    }
    let Some(line) = lines.iter().find(|l| solid(l)) else { return clipped };
    let (w, h) = (width(line).min(width(&work)), height(line).min(height(&work)));
    let left = i64::from(line.left).clamp(i64::from(work.left), i64::from(work.right) - w);
    let top = i64::from(line.top).clamp(i64::from(work.top), i64::from(work.bottom) - h);
    vec![Rect { left: narrow(left), top: narrow(top), right: narrow(left + w), bottom: narrow(top + h) }]
}

/// Whether a text reads right to left: its first strong character is Hebrew, Arabic or one of their presentation forms. Input: the text. Return: the answer.
pub fn is_rtl(text: &str) -> bool {
    let strong = text.chars().find(|c| c.is_alphabetic());
    strong.is_some_and(|c| matches!(c, '\u{590}'..='\u{8FF}' | '\u{FB1D}'..='\u{FDFF}' | '\u{FE70}'..='\u{FEFF}'))
}

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

/// The ideal top-left of the card on `side`: below the last line, above the first, or beside the first. A selection's card is centred on the line's middle, as the native card was, so its arrow points there from the middle of its edge; a caret's or the pointer's card (no arrow) has its left edge on the line's start (its right edge on the line's end when right to left) and its top on the line's top. Input: the side, the first and last lines, the card's size, the gap, whether the card is centred and the direction. Return: the corner and the arrow's coordinate on the selection.
fn ideal(side: Side, (first, last): (Rect, Rect), size: (i64, i64), gap: i64, centred: bool, rtl: bool) -> (i64, i64, i64) {
    let line = if side == Side::Below { last } else { first };
    let centre_x = (i64::from(line.left) + i64::from(line.right)) / 2;
    let centre_y = (i64::from(line.top) + i64::from(line.bottom)) / 2;
    let across = match (centred, rtl) {
        (true, _) => centre_x - size.0 / 2,
        (false, true) => i64::from(line.right) - size.0,
        (false, false) => i64::from(line.left),
    };
    let down = if centred { centre_y - size.1 / 2 } else { i64::from(line.top) };
    match side {
        Side::Below => (across, i64::from(line.bottom) + gap, centre_x),
        Side::Above => (across, i64::from(line.top) - gap - size.1, centre_x),
        Side::Right => (i64::from(line.right) + gap, down, centre_y),
        _ => (i64::from(line.left) - gap - size.0, down, centre_y),
    }
}

/// Scores one side: moves the ideal corner into `area` and compares. Only a move toward or away from the selection counts against it; sliding along the selection (a centred card near the screen's edge) is free, as on the native card.
fn candidate(order: i64, side: Side, req: &Request, area: &Rect, ends: (Rect, Rect), size: (i64, i64)) -> Candidate {
    let selection = req.kind == Kind::Selection;
    let gap = (if selection { GAP_SELECTION } else { GAP_POINT } * sane(req.scale)).round() as i64;
    let (x, y, target) = ideal(side, ends, size, gap, selection, req.rtl);
    let left = x.clamp(i64::from(area.left), i64::from(area.right) - size.0);
    let top = y.clamp(i64::from(area.top), i64::from(area.bottom) - size.1);
    let rect = Rect { left: narrow(left), top: narrow(top), right: narrow(left + size.0), bottom: narrow(top + size.1) };
    let pushed = if matches!(side, Side::Below | Side::Above) { top - y } else { left - x };
    let covers = req.lines.iter().any(|l| shared(l, &rect) > 0);
    Candidate { side, rect, target, score: (pushed == 0, !covers, shared(&rect, area), -pushed.abs(), -order) }
}

/// Places a card beside a selection. Input: the request. Return: None when the work area is empty or no line is valid; otherwise the best of the four sides, always inside the work area, never larger than it.
pub fn place(req: &Request) -> Option<Placed> {
    let area = inner(req.work, sane(req.scale))?;
    let valid: Vec<Rect> = req.lines.iter().copied().filter(solid).collect();
    let ends = ends(&valid)?;
    let size = fitted(req.size, &area);
    let req = Request { lines: &valid, ..*req };
    let best = [Side::Below, Side::Above, Side::Right, Side::Left]
        .into_iter()
        .enumerate()
        .map(|(order, side)| candidate(order as i64, side, &req, &area, ends, size))
        .max_by_key(|c| c.score)?;
    Some(Placed { rect: best.rect, side: best.side, target: (req.kind == Kind::Selection).then(|| narrow(best.target)), rtl: req.rtl })
}

/// Places a window with no selection in the middle of the work area. Input: the work area, the wanted size and the scale. Return: None for an empty work area; the window is shrunk to the area when it is larger.
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
    /// Where the arrow's middle sits on the card's edge next to the selection, in physical pixels from the edge's start (left for below and above, top for right and left): at the selection's middle, kept `inset` from the corners (only a card pushed by the screen's edge needs that) so it clears their rounding. None for a card with no arrow (a caret, the pointer, no selection).
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
