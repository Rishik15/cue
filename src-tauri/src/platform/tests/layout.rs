//! File: platform/tests/layout.rs
//! Purpose: Protect the card layout: words wrap inside the card without losing or reordering any, and the card lands under the selection when there is room, over it, beside it or squeezed in, always inside the screen.
//! Contents: tests of `flow`, `place` and `fit_logical`.

use crate::platform::flow::{flow, Piece, Placed as Word};
use crate::platform::place::{fit_logical, place, Placed, Side};
use crate::platform::Rect;

/// A rectangle in pixels. Inputs: left, top, right, bottom. Returns it.
fn rect(left: i32, top: i32, right: i32, bottom: i32) -> Rect {
    Rect { left, top, right, bottom }
}

/// Wraps words each ten pixels per letter with a five pixel gap. Inputs: the words, the indents and the right limit. Returns the places, the lines and the end of the last line.
fn wrapped(words: &[&str], first_x: i32, next_x: i32, right: i32) -> (Vec<Word>, usize, i32) {
    let pieces: Vec<Piece> = words.iter().map(|w| Piece { text: w, style: 0 }).collect();
    flow(&pieces, first_x, next_x, right, |text, _| text.len() as i32 * 10, |_| 5)
}

/// Words fill a line up to the limit, the next one starts the next line at the later indent, and no word is lost or moved.
#[test]
fn words_wrap_at_the_limit_and_keep_their_order() {
    let (placed, lines, end) = wrapped(&["aa", "bb", "cc", "dd"], 0, 4, 50);
    let at = |piece, x, line| Word { piece, x, line };
    assert_eq!(placed, [at(0, 0, 0), at(1, 25, 0), at(2, 4, 1), at(3, 29, 1)]);
    assert_eq!((lines, end), (2, 49));
}

/// A word wider than the line sits alone on its own line; an empty paragraph is one empty line; the first line may start further in.
#[test]
fn a_too_wide_word_sits_alone_and_nothing_is_one_empty_line() {
    let (placed, lines, _) = wrapped(&["abcdefgh", "a"], 0, 0, 30);
    assert_eq!((placed[0].line, placed[0].x, placed[1].line, placed[1].x, lines), (0, 0, 1, 0, 2));
    assert_eq!(wrapped(&[], 12, 0, 30), (vec![], 1, 12));
    assert_eq!(wrapped(&["a"], 12, 0, 30).0[0].x, 12);
}

/// The card goes under the selection, centred on it, with its arrow pointing up at it.
#[test]
fn the_card_goes_under_the_selection_when_there_is_room() {
    let placed = place(rect(400, 300, 500, 320), rect(0, 0, 1000, 800), (200, 100), 12, 16);
    assert_eq!(placed, Placed { x: 350, y: 332, side: Side::Top, offset: 100 });
}

/// With no room below it goes over, with no room above or below it goes beside, right first and then left.
#[test]
fn the_card_moves_over_then_beside_the_selection() {
    let work = rect(0, 0, 1000, 800);
    let over = place(rect(400, 700, 500, 720), work, (200, 100), 12, 16);
    assert_eq!((over.side, over.y), (Side::Bottom, 588));
    let right = place(rect(400, 10, 500, 790), work, (200, 100), 12, 16);
    assert_eq!((right.side, right.x), (Side::Left, 512));
    let left = place(rect(800, 10, 900, 790), work, (200, 100), 12, 16);
    assert_eq!((left.side, left.x), (Side::Right, 588));
}

/// Near a screen edge the card is kept 8 pixels inside it and the arrow keeps clear of the corner; a selection that fills the screen still gets a card inside it.
#[test]
fn the_card_stays_inside_the_screen() {
    let work = rect(0, 0, 1000, 800);
    let edge = place(rect(0, 300, 20, 320), work, (200, 100), 12, 16);
    assert_eq!((edge.x, edge.side, edge.offset), (8, Side::Top, 16));
    let everything = place(rect(0, 0, 1000, 800), work, (200, 100), 12, 16);
    assert_eq!(everything, Placed { x: 400, y: 692, side: Side::Top, offset: 100 });
}

/// A window shrinks to the work area (in logical pixels, with a margin) and never grows or vanishes.
#[test]
fn windows_shrink_to_fit_the_work_area_and_never_vanish() {
    let area = rect(0, 0, 1920, 1040);
    assert_eq!(fit_logical((2000.0, 500.0), area, 1.5, 10.0), (1260.0, 500.0));
    let (w, h) = fit_logical((800.0, 900.0), area, 1.5, 10.0);
    assert_eq!(w, 800.0);
    assert!((h - 673.333).abs() < 0.01, "{h}");
    assert_eq!(fit_logical((300.0, 300.0), rect(0, 0, 10, 10), 1.0, 20.0), (1.0, 1.0));
}
