//! File: dictionary/placement/display.rs
//! Purpose: Which monitor a selection is on and the selection's lines cut to that monitor's work area.
//! Contents: choose_display — the monitor a selection is on; gap_between — the distance between rectangles that do not overlap; clip_lines — the lines cut to a work area.

use super::rect::{height, intersect, narrow, shared, solid, width};
use crate::platform::Rect;

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
