//! Purpose: Where a floating card goes and which way its arrow points: under the selected text when there is room, else over it, else beside it, kept inside the work area of its monitor.
//! Contents: Side — the card edge the arrow sits on; Placed — the card's top-left corner, the side and the arrow's distance along that edge; place — choose the first side that fits.

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

#[derive(Debug, PartialEq)]
pub struct Placed {
    pub x: i32,
    pub y: i32,
    pub side: Side,
    /// Distance of the arrow's middle from the start of its edge (left for top and bottom, top for left and right).
    pub offset: i32,
}

/// `reach` is the space the arrow and its gap take between the selection and the card; `inset` keeps the arrow clear of the card's rounded corners.
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

#[cfg(test)]
mod tests {
    use super::*;

    const WORK: Rect = Rect { left: 0, top: 0, right: 1920, bottom: 1040 };
    const CARD: (i32, i32) = (340, 200);

    fn word(x: i32, y: i32) -> Rect {
        Rect { left: x - 20, top: y - 10, right: x + 20, bottom: y + 10 }
    }

    #[test]
    fn opens_below_with_the_arrow_on_top() {
        let p = place(word(800, 300), WORK, CARD, 14, 20);
        assert_eq!((p.x, p.y, p.side, p.offset), (630, 324, Side::Top, 170));
    }

    #[test]
    fn flips_above_near_the_bottom() {
        let p = place(word(800, 1000), WORK, CARD, 14, 20);
        assert_eq!((p.y, p.side), (990 - 14 - 200, Side::Bottom));
    }

    #[test]
    fn goes_beside_the_text_when_there_is_no_room_above_or_below() {
        let tall = Rect { left: 700, top: 0, right: 760, bottom: 1040 };
        let p = place(tall, WORK, CARD, 14, 20);
        assert_eq!((p.x, p.side), (774, Side::Left));
        let at_right = Rect { left: 1700, top: 0, right: 1900, bottom: 1040 };
        assert_eq!(place(at_right, WORK, CARD, 14, 20).side, Side::Right);
    }

    #[test]
    fn stays_inside_the_screen_and_the_arrow_stays_on_the_card() {
        let p = place(word(5, 300), WORK, CARD, 14, 20);
        assert_eq!((p.x, p.offset), (8, 20)); // pushed right by the edge, the arrow as near the selection as the corner allows
        let q = place(word(1915, 300), WORK, CARD, 14, 20);
        assert_eq!((q.x, q.offset), (1920 - 340 - 8, 340 - 20));
    }

    #[test]
    fn a_card_that_fits_nowhere_goes_below_pinned_inside_the_work_area() {
        let tight = Rect { left: 0, top: 0, right: 300, bottom: 150 };
        let p = place(word(150, 75), tight, CARD, 14, 20);
        assert_eq!((p.side, p.y), (Side::Top, 8)); // pushed up to the top edge margin; the card is taller than the area
        assert!(p.x >= 8);
    }
}
