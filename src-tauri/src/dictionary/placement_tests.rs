//! File: dictionary/placement_tests.rs
//! Purpose: Protect the card's placement with the cases of the placement research (physical coordinates, negative origins, scales, tiny and huge input, right to left, tall selections, each-side resize) and a property test that a card is always inside the work area.
//! Contents: tests of `to_px`, `anchor`, `place`, `centred`, `resize`, `choose_display`, `clip_lines`, `is_rtl` and `Placed::arrow`.

use proptest::prelude::*;

use super::placement::{
    anchor, centred, choose_display, clip_lines, is_rtl, place, resize, to_px, Kind, Placed, Request, Side, ARROW_INSET, ARROW_OFFSET,
};
use crate::platform::Rect;

/// A rectangle from its four sides.
const fn r(left: i32, top: i32, right: i32, bottom: i32) -> Rect {
    Rect { left, top, right, bottom }
}

const WORK: Rect = r(0, 0, 1920, 1080);

/// A selection request for a 300 by 200 card on the standard work area at scale 1.
fn ask(lines: &[Rect]) -> Request<'_> {
    Request { lines, kind: Kind::Selection, work: WORK, size: (300, 200), scale: 1.0, rtl: false }
}

/// Whether `inner` lies inside `outer`.
fn inside(inner: &Rect, outer: &Rect) -> bool {
    inner.left >= outer.left && inner.top >= outer.top && inner.right <= outer.right && inner.bottom <= outer.bottom
}

/// Whether two rectangles share area.
fn overlap(a: &Rect, b: &Rect) -> bool {
    a.left < b.right && b.left < a.right && a.top < b.bottom && b.top < a.bottom
}

/// With room everywhere the card is below the line, 11 px away, its arrow 36 px from its left edge on the line's middle.
#[test]
fn a_single_line_with_room_gets_the_card_below_at_the_gap() {
    let line = r(500, 300, 560, 320);
    let got = place(&ask(&[line])).expect("placed");
    assert_eq!((got.side, got.rect), (Side::Below, r(494, 331, 794, 531)));
    assert_eq!(got.target, Some(530));
}

/// Near the bottom edge the card goes above, its bottom 11 px over the line.
#[test]
fn a_line_at_the_bottom_edge_gets_the_card_above() {
    let got = place(&ask(&[r(500, 1050, 560, 1070)])).expect("placed");
    assert_eq!((got.side, got.rect.bottom), (Side::Above, 1039));
}

/// With no vertical room the card goes beside the line: right when it fits there, left when the line is at the right edge.
#[test]
fn without_vertical_room_the_card_goes_to_a_side() {
    let work = r(0, 0, 1920, 260);
    let at = |line: Rect| place(&Request { work, ..ask(&[line]) }).expect("placed");
    assert_eq!(at(r(500, 100, 560, 120)).side, Side::Right);
    let left = at(r(1800, 100, 1860, 120));
    assert_eq!(left.side, Side::Left);
    assert!(left.rect.right <= 1789);
}

/// When nothing fits cleanly the card is still inside the work area, off the line when it can be, and the same input always gives the same answer.
#[test]
fn a_cramped_work_area_still_gets_a_card_inside_it() {
    let work = r(0, 0, 400, 260);
    let line = r(20, 100, 60, 120);
    let lines = [line];
    let req = Request { work, ..ask(&lines) };
    let first = place(&req).expect("placed");
    assert!(inside(&first.rect, &work));
    assert!(!overlap(&first.rect, &line));
    assert_eq!(place(&req), Some(first));
}

/// A card larger than the work area shrinks to it (inside its margin) and stays positive.
#[test]
fn a_card_larger_than_the_work_area_shrinks_to_it() {
    let got = place(&Request { size: (3000, 3000), ..ask(&[r(500, 300, 560, 320)]) }).expect("placed");
    assert_eq!(got.rect, r(8, 8, 1912, 1072));
    let tiny = r(0, 0, 100, 80);
    let got = place(&Request { work: tiny, ..ask(&[r(10, 10, 50, 30)]) }).expect("placed");
    assert!(inside(&got.rect, &tiny) && got.rect.right > got.rect.left && got.rect.bottom > got.rect.top);
}

/// Below anchors to the last line and above to the first, and the card covers no line of a wrapped selection.
#[test]
fn a_wrapped_selection_anchors_to_its_last_and_first_lines() {
    let lines = [r(100, 100, 500, 120), r(100, 120, 300, 140)];
    let below = place(&ask(&lines)).expect("below");
    assert_eq!((below.side, below.rect.top), (Side::Below, 151));
    let low = [r(100, 1000, 500, 1020), r(100, 1020, 300, 1040)];
    let above = place(&ask(&low)).expect("above");
    assert_eq!((above.side, above.rect.bottom), (Side::Above, 989));
    assert!(low.iter().all(|l| !overlap(l, &above.rect)));
}

/// Two distant paragraphs: the card sits at the last line, never at the middle of their union, and the monitor with more of the lines wins.
#[test]
fn distant_paragraphs_anchor_on_a_line_not_on_their_union() {
    let lines = [r(100, 100, 200, 120), r(100, 700, 200, 720)];
    let got = place(&ask(&lines)).expect("placed");
    assert_eq!((got.side, got.rect.top), (Side::Below, 731));
    let works = [r(-1920, 0, 0, 1080), r(0, 0, 1920, 1080)];
    assert_eq!(choose_display(&[r(-100, 100, 50, 120)], &works), Some(0));
    assert_eq!(choose_display(&[r(-100, 100, 50, 120), r(0, 300, 400, 320)], &works), Some(1));
}

/// Right to left aligns a caret's card's right edge with the line's end and puts a selection card's arrow 36 px from its right edge, and the first strong letter tells the direction.
#[test]
fn right_to_left_aligns_the_right_edge() {
    let caret = place(&Request { rtl: true, kind: Kind::Caret, ..ask(&[r(900, 300, 1000, 320)]) }).expect("placed");
    assert_eq!((caret.side, caret.rect.right), (Side::Below, 1000));
    let selection = place(&Request { rtl: true, ..ask(&[r(900, 300, 1000, 320)]) }).expect("placed");
    assert_eq!((selection.rect.right, selection.arrow(34)), (986, Some(264)));
    assert!(is_rtl("\u{5e9}\u{5dc}\u{5d5}\u{5dd} world") && is_rtl("123 \u{627}\u{644}") && !is_rtl("hello \u{5e9}") && !is_rtl("123"));
}

/// Lines off one monitor, between two, or on none are clipped or moved onto the chosen work area; the nearest monitor wins when none overlaps.
#[test]
fn lines_are_clipped_to_the_chosen_monitor() {
    let works = [r(-1920, 0, 0, 1080), r(0, 0, 1920, 1080)];
    assert_eq!(choose_display(&[r(5000, 100, 5100, 120)], &works), Some(1));
    assert_eq!(choose_display(&[r(-5000, 100, -4900, 120)], &works), Some(0));
    assert_eq!(choose_display(&[r(1, 1, 2, 2)], &[r(0, 0, 0, 0)]), None);
    assert_eq!(clip_lines(&[r(-100, 100, 50, 120)], works[1]), [r(0, 100, 50, 120)]);
    let moved = clip_lines(&[r(5000, 100, 5100, 120)], works[1]);
    assert_eq!(moved, [r(1820, 100, 1920, 120)]);
    assert!(clip_lines(&[r(0, 0, 0, 0)], works[1]).is_empty());
}

/// A work area with a negative origin and the taskbar cut out is never left, and the card stays on its own monitor.
#[test]
fn a_negative_origin_and_a_taskbar_are_respected() {
    for work in [r(-1920, 0, 0, 1040), r(-1920, 40, 0, 1080), r(-1880, 0, 0, 1080), r(0, 0, 1880, 1080)] {
        let line = r(work.left + 400, work.bottom - 30, work.left + 460, work.bottom - 10);
        let got = place(&Request { work, ..ask(&[line]) }).expect("placed");
        assert!(inside(&got.rect, &work), "{work:?} {got:?}");
        assert!(!overlap(&got.rect, &line));
    }
}

/// The scale changes the gap and the margin in physical pixels and nothing else: a 200 percent monitor puts the card 22 px away.
#[test]
fn the_scale_sets_the_gap_in_physical_pixels() {
    let line = r(500, 300, 560, 320);
    let at = |scale: f64| place(&Request { scale, ..ask(&[line]) }).expect("placed").rect.top;
    assert_eq!([at(1.0), at(1.5), at(2.0), at(3.0)], [331, 337, 342, 353]);
    assert_eq!(at(f64::NAN), 331);
    assert_eq!(at(1e9), 331);
}

/// A card that is too small for the work area stays inside it at every common scale, and its anchor edge is fixed when it grows.
#[test]
fn a_tiny_work_area_shrinks_the_card_and_resizing_keeps_its_anchor_edge() {
    let work = r(0, 0, 200, 150);
    let line = r(20, 20, 60, 40);
    let lines = [line];
    let req = Request { work, scale: 2.0, ..ask(&lines) };
    let first = place(&req).expect("placed");
    assert!(inside(&first.rect, &work));
    let grown = resize(&first, (900, 900), work, 2.0).expect("resized");
    assert!(inside(&grown.rect, &work));
    assert_eq!(grown.side, first.side);
}

/// A tall selection (a large font) that leaves no room above or below puts the card beside it, its arrow 36 px from its top on the first line's middle.
#[test]
fn a_tall_selection_gets_the_card_beside_it() {
    let line = r(800, 100, 900, 1000);
    let got = place(&ask(&[line])).expect("placed");
    assert_eq!((got.side, got.rect.left, got.rect.top), (Side::Right, 911, 514));
    assert_eq!(got.target, Some(550));
}

/// A caret or the pointer gets a tighter gap and no arrow; the same lines as a selection get the arrow.
#[test]
fn a_caret_or_the_pointer_has_a_tight_gap_and_no_arrow() {
    let line = r(500, 300, 560, 320);
    for kind in [Kind::Caret, Kind::Cursor] {
        let got = place(&Request { kind, ..ask(&[line]) }).expect("placed");
        assert_eq!((got.rect.top, got.target, got.arrow(12)), (324, None, None));
    }
    assert!(place(&ask(&[line])).expect("placed").arrow(12).is_some());
}

/// Hostile input gives nothing or a card inside the area, never a panic or an overflow.
#[test]
fn extreme_and_empty_input_is_refused_or_contained() {
    assert!(place(&ask(&[])).is_none());
    assert!(place(&ask(&[r(5, 5, 5, 9), r(9, 9, 2, 2)])).is_none());
    assert!(place(&Request { work: r(0, 0, 0, 0), ..ask(&[r(1, 1, 5, 5)]) }).is_none());
    let huge = r(i32::MIN, i32::MIN, i32::MAX, i32::MAX);
    let got = place(&Request { work: huge, size: (i32::MAX, i32::MAX), ..ask(&[huge]) }).expect("placed");
    assert!(got.rect.right >= got.rect.left && got.rect.bottom >= got.rect.top);
    let got = place(&Request { size: (i32::MIN, -4), ..ask(&[r(1, 1, 5, 5)]) }).expect("placed");
    assert!(inside(&got.rect, &WORK) && got.rect.right > got.rect.left);
    assert!(centred(r(0, 0, 0, 0), (1, 1), 1.0).is_none());
}

/// Resizing moves only the far edge: below keeps its top, above its bottom, right its left, left its right; a size the room cannot hold is cut, not pushed over the text.
#[test]
fn resizing_keeps_the_edge_next_to_the_selection() {
    let at = |side, rect| Placed { rect, side, target: Some(100), rtl: false };
    let size = (360, 300);
    let below = resize(&at(Side::Below, r(500, 330, 800, 530)), size, WORK, 1.0).expect("below");
    assert_eq!((below.rect.left, below.rect.top, below.rect.bottom), (500, 330, 630));
    let above = resize(&at(Side::Above, r(500, 700, 800, 900)), size, WORK, 1.0).expect("above");
    assert_eq!((above.rect.left, above.rect.bottom, above.rect.top), (500, 900, 600));
    let right = resize(&at(Side::Right, r(910, 100, 1210, 300)), size, WORK, 1.0).expect("right");
    assert_eq!((right.rect.left, right.rect.top, right.rect.right), (910, 100, 1270));
    let left = resize(&at(Side::Left, r(500, 100, 800, 300)), size, WORK, 1.0).expect("left");
    assert_eq!((left.rect.right, left.rect.top, left.rect.left), (800, 100, 440));
    let cut = resize(&at(Side::Below, r(500, 900, 800, 1000)), (300, 900), WORK, 1.0).expect("cut");
    assert_eq!((cut.rect.top, cut.rect.bottom), (900, 1072));
    let rtl = resize(&Placed { rtl: true, ..at(Side::Below, r(700, 330, 1000, 530)) }, (400, 200), WORK, 1.0).expect("rtl");
    assert_eq!((rtl.rect.right, rtl.rect.left), (1000, 600));
}

/// The search window is centred, shrunk to a small work area and has no arrow.
#[test]
fn the_search_window_is_centred() {
    let got = centred(WORK, (392, 512), 1.0).expect("centred");
    assert_eq!((got.side, got.rect, got.arrow(12)), (Side::Centre, r(764, 284, 1156, 796), None));
    let small = centred(r(0, 0, 300, 300), (392, 512), 1.0).expect("small");
    assert_eq!(small.rect, r(8, 8, 292, 292));
}

/// The arrow stays 12 px from the corners, and where the selection is when that is further in.
#[test]
fn the_arrow_keeps_clear_of_the_corners() {
    let card = |target| Placed { rect: r(500, 330, 800, 530), side: Side::Below, target: Some(target), rtl: false };
    assert_eq!(card(520).arrow(12), Some(20));
    assert_eq!(card(100).arrow(12), Some(12));
    assert_eq!(card(5000).arrow(12), Some(288));
}

proptest! {
    /// Whatever the lines, area, size and scale, a placed card is inside the work area (or is the whole area), positive in size, and never panics.
    #[test]
    fn a_placed_card_is_always_inside_the_work_area(
        lines in prop::collection::vec((any::<i32>(), any::<i32>(), any::<i32>(), any::<i32>()), 0..5),
        work in (-3000i32..3000, -3000i32..3000, 1i32..4000, 1i32..4000),
        size in (any::<i32>(), any::<i32>()),
        scale in prop::num::f64::ANY,
        rtl in any::<bool>(),
    ) {
        let lines: Vec<Rect> = lines.into_iter().map(|(a, b, c, d)| r(a, b, c, d)).collect();
        let work = r(work.0, work.1, work.0 + work.2, work.1 + work.3);
        if let Some(got) = place(&Request { lines: &lines, kind: Kind::Selection, work, size, scale, rtl }) {
            prop_assert!(inside(&got.rect, &work));
            prop_assert!(got.rect.right > got.rect.left && got.rect.bottom > got.rect.top);
            let again = resize(&got, size, work, scale).expect("resize");
            prop_assert!(inside(&again.rect, &work));
        }
    }
}

/// A card taller than any limit the page may ask for is cut to the limit (placement then shrinks it to the room and the page scrolls); a size that is not a length is refused.
#[test]
fn a_very_tall_card_is_cut_not_refused() {
    assert_eq!(to_px(2400.0, 1.0), Some(1600));
    assert_eq!(to_px(2400.0, 1.5), Some(2400));
    assert_eq!(to_px(300.0, 1.25), Some(375));
    for bad in [0.0, 0.5, -3.0, f64::NAN, f64::INFINITY] {
        assert_eq!(to_px(bad, 1.0), None, "{bad}");
    }
    let tall = to_px(2400.0, 1.0).unwrap();
    let placed = place(&Request { size: (300, tall), ..ask(&[r(100, 100, 160, 120)]) }).expect("placed");
    assert!(inside(&placed.rect, &WORK));
}

/// A caret with no width or height cannot be placed beside, so the pointer's box is used instead of closing the card; a usable caret is still preferred to the pointer, and a selection to both.
#[test]
fn an_empty_caret_falls_back_to_the_pointer() {
    let pointer = r(500, 500, 524, 520);
    for caret in [r(10, 10, 10, 30), r(10, 10, 12, 10), r(10, 10, 10, 10)] {
        assert_eq!(anchor(&[], Some(caret), Some(pointer)), Some((vec![pointer], Kind::Cursor)), "{caret:?}");
    }
    let caret = r(10, 10, 12, 30);
    assert_eq!(anchor(&[], Some(caret), Some(pointer)), Some((vec![caret], Kind::Caret)));
    let line = r(50, 50, 90, 70);
    assert_eq!(anchor(&[line], Some(caret), Some(pointer)), Some((vec![line], Kind::Selection)));
    assert_eq!(anchor(&[], None, None), None);
}

/// A one-line selection placed on `side` at `scale`: the work area and line (in dip, scaled to physical pixels) that leave only that side free. Input: the side and the scale. Return: the work area and the line in physical pixels.
fn only_room_on(side: Side, scale: f64) -> (Rect, Rect) {
    let px = |v: f64| (v * scale).round() as i32;
    let (work_h, left, top) = match side {
        Side::Below => (1080.0, 500.0, 300.0),
        Side::Above => (1080.0, 500.0, 1040.0),
        Side::Right => (320.0, 500.0, 120.0),
        _ => (320.0, 1840.0, 120.0),
    };
    (r(0, 0, px(1920.0), px(work_h)), r(px(left), px(top), px(left + 60.0), px(top + 20.0)))
}

/// On every side at 100, 150 and 200 percent, left to right and right to left, the arrow's middle is at the middle of the selected line (within half a pixel), 36 dip from the card's start edge (its right edge for a right-to-left card below or above), and the card's edge is the 11 dip gap from the line.
#[test]
fn the_arrow_points_at_the_middle_of_the_line_on_every_side_and_scale() {
    for (scale, rtl) in [1.0, 1.5, 2.0].into_iter().flat_map(|s| [(s, false), (s, true)]) {
        let px = |v: f64| (v * scale).round() as i32;
        for side in [Side::Below, Side::Above, Side::Right, Side::Left] {
            let (work, line) = only_room_on(side, scale);
            let req = Request { lines: &[line], kind: Kind::Selection, work, size: (px(300.0), px(200.0)), scale, rtl };
            let got = place(&req).expect("placed");
            assert_eq!(got.side, side, "{scale}");
            let arrow = got.arrow(px(ARROW_INSET)).expect("arrow");
            let c = got.rect;
            let (start, middle, length, gap) = match side {
                Side::Below => (c.left, (line.left + line.right) / 2, c.right - c.left, c.top - line.bottom),
                Side::Above => (c.left, (line.left + line.right) / 2, c.right - c.left, line.top - c.bottom),
                Side::Right => (c.top, (line.top + line.bottom) / 2, c.bottom - c.top, c.left - line.right),
                _ => (c.top, (line.top + line.bottom) / 2, c.bottom - c.top, line.left - c.right),
            };
            assert_eq!(start + arrow, middle, "{scale} {side:?} {c:?}");
            let exact = match side {
                Side::Below | Side::Above => (f64::from(line.left) + f64::from(line.right)) / 2.0 - f64::from(start + arrow),
                _ => (f64::from(line.top) + f64::from(line.bottom)) / 2.0 - f64::from(start + arrow),
            };
            assert!(exact.abs() <= 0.5, "{scale} {side:?}");
            let from_start = if rtl && matches!(side, Side::Below | Side::Above) { length - arrow } else { arrow };
            assert_eq!(from_start, px(ARROW_OFFSET), "{scale} {side:?} {rtl}");
            assert_eq!(gap, px(11.0), "{scale} {side:?}");
        }
    }
}

/// A card pushed by the screen's edge keeps its arrow the inset from its corner at every scale: a word at the very left edge gets the arrow 34 dip in, as near the word as the rounding allows.
#[test]
fn a_card_pushed_by_the_screen_edge_keeps_its_arrow_off_the_corner() {
    for scale in [1.0, 1.5, 2.0] {
        let px = |v: f64| (v * scale).round() as i32;
        let line = r(0, px(300.0), px(20.0), px(320.0));
        let req = Request {
            lines: &[line],
            kind: Kind::Selection,
            work: r(0, 0, px(1920.0), px(1080.0)),
            size: (px(300.0), px(200.0)),
            scale,
            rtl: false,
        };
        let got = place(&req).expect("placed");
        assert_eq!((got.side, got.rect.left), (Side::Below, px(8.0)), "{scale}");
        assert_eq!(got.arrow(px(ARROW_INSET)), Some(px(34.0)), "{scale}");
    }
}

/// A card pushed in by the right screen edge slides left and its arrow slides along its edge, the tip still on the word, at every scale.
#[test]
fn a_card_pushed_by_the_right_edge_slides_its_arrow_to_stay_on_the_word() {
    for scale in [1.0, 1.5, 2.0] {
        let px = |v: f64| (v * scale).round() as i32;
        let line = r(px(1690.0), px(300.0), px(1710.0), px(320.0));
        let req = Request {
            lines: &[line],
            kind: Kind::Selection,
            work: r(0, 0, px(1920.0), px(1080.0)),
            size: (px(300.0), px(200.0)),
            scale,
            rtl: false,
        };
        let got = place(&req).expect("placed");
        let arrow = got.arrow(px(ARROW_INSET)).expect("arrow");
        assert_eq!((got.side, got.rect.right), (Side::Below, px(1912.0)), "{scale}");
        assert_eq!(got.rect.left + arrow, (line.left + line.right) / 2, "{scale}");
        assert!(arrow > px(ARROW_OFFSET), "{scale}");
    }
}

/// A right-to-left card below a word near the left screen edge, and a card beside a word near the bottom edge, are pushed in and their arrows slide along their edge with the tip still on the word, at every scale.
#[test]
fn a_pushed_right_to_left_or_side_card_slides_its_arrow_to_stay_on_the_word() {
    for scale in [1.0, 1.5, 2.0] {
        let px = |v: f64| (v * scale).round() as i32;
        let rtl_line = r(px(120.0), px(300.0), px(140.0), px(320.0));
        let req = Request {
            lines: &[rtl_line],
            kind: Kind::Selection,
            work: r(0, 0, px(1920.0), px(1080.0)),
            size: (px(300.0), px(200.0)),
            scale,
            rtl: true,
        };
        let got = place(&req).expect("rtl");
        let arrow = got.arrow(px(ARROW_INSET)).expect("arrow");
        assert_eq!((got.side, got.rect.left), (Side::Below, px(8.0)), "{scale}");
        assert_eq!(got.rect.left + arrow, (rtl_line.left + rtl_line.right) / 2, "{scale}");
        let side_line = r(px(500.0), px(200.0), px(560.0), px(220.0));
        let req = Request {
            lines: &[side_line],
            kind: Kind::Selection,
            work: r(0, 0, px(1920.0), px(320.0)),
            size: (px(300.0), px(200.0)),
            scale,
            rtl: false,
        };
        let got = place(&req).expect("side");
        let arrow = got.arrow(px(ARROW_INSET)).expect("arrow");
        assert_eq!((got.side, got.rect.bottom), (Side::Right, px(312.0)), "{scale}");
        assert_eq!(got.rect.top + arrow, (side_line.top + side_line.bottom) / 2, "{scale}");
        assert!(arrow > px(ARROW_OFFSET), "{scale}");
    }
}
