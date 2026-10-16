//! Purpose: Where the definition popup goes: beside the pointer, kept inside the work area of its monitor.
//! Contents: place — top-left corner for a popup of a given size (below the pointer, or above it when there is no room below).

use crate::platform::{Point, Rect};

/// Space kept between the popup and the screen edge.
const EDGE: i32 = 8;

pub fn place(anchor: Point, work: Rect, size: (i32, i32), gap: i32) -> (i32, i32) {
    let (w, h) = size;
    let x = (anchor.x - w / 2).clamp(work.left + EDGE, (work.right - w - EDGE).max(work.left + EDGE));
    let below = anchor.y + gap;
    let y = if below + h + EDGE <= work.bottom { below } else { anchor.y - gap - h };
    (x, y.clamp(work.top + EDGE, (work.bottom - h - EDGE).max(work.top + EDGE)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORK: Rect = Rect { left: 0, top: 0, right: 1920, bottom: 1040 };

    #[test]
    fn opens_below_the_pointer() {
        assert_eq!(place(Point { x: 800, y: 300 }, WORK, (380, 200), 20), (610, 320));
    }

    #[test]
    fn flips_above_near_the_bottom() {
        assert_eq!(place(Point { x: 800, y: 1000 }, WORK, (380, 200), 20), (610, 780));
    }

    #[test]
    fn stays_inside_the_screen_sideways() {
        assert_eq!(place(Point { x: 5, y: 300 }, WORK, (380, 200), 20).0, 8);
        assert_eq!(place(Point { x: 1915, y: 300 }, WORK, (380, 200), 20).0, 1920 - 380 - 8);
    }
}
