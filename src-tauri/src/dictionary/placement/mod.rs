//! File: dictionary/placement/mod.rs
//! Purpose: Where the dictionary card goes beside a selection (and the search window beside the pointer), as pure geometry in physical virtual-screen pixels: no Tauri, no Windows. Negative origins, any monitor scale and hostile numbers are handled here, so the window code only converts once at its boundary and sets the result.
//! Contents: Side / Kind / Request / Placed — the vocabulary; ARROW_OFFSET / ARROW_INSET — where the arrow sits; to_px — a page's size in pixels; anchor — what a card points at; is_rtl — whether a text reads right to left; the public face of `display` (choose_display, clip_lines), `geometry` (place, centred, resize, `Placed::arrow`) and `rect` (grown).
//! Candidates are scored in order: the card keeps its gap from the selection without being pushed (fits), it covers no selection line, more of it is on screen, it moves less toward or away from the selection, then the fixed order below, above, right, left. A window larger than the work area shrinks to it and its page scrolls.

mod display;
mod geometry;
mod rect;

use crate::platform::Rect;

pub use display::{choose_display, clip_lines};
pub use geometry::{centred, place, resize};
pub use rect::grown;

/// Which side of the selection (or the pointer) the window is on; `Centre` is the search window when the pointer could not be read.
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

/// Whether a text reads right to left: its first strong character is Hebrew, Arabic or one of their presentation forms. Input: the text. Return: the answer.
pub fn is_rtl(text: &str) -> bool {
    let strong = text.chars().find(|c| c.is_alphabetic());
    strong.is_some_and(|c| matches!(c, '\u{590}'..='\u{8FF}' | '\u{FB1D}'..='\u{FDFF}' | '\u{FE70}'..='\u{FEFF}'))
}

/// Where the arrow's middle sits from the card's start edge, in dip, as `design.html` draws the card (its notch 36 dip from the left edge): from the left edge for a card below or above the text (the right edge right to left), from the top edge for a card beside it. The card is placed so the arrow's tip is on the middle of the line and the card extends away from there.
pub const ARROW_OFFSET: f64 = 36.0;
/// How far the arrow's middle keeps from a card's corners, in dip: the card's 16 dip corner radius and the 18 dip half width of the notch with its shoulders, so the notch always stands on the straight edge.
pub const ARROW_INSET: f64 = 34.0;

/// The longest side the page may ask for, in dip: the search window is 360 by 360 and a card is 300 wide, so more than this is a card taller than any screen, which is cut to the work area and scrolls.
const MAX_SIDE: f64 = 1600.0;
