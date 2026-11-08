//! File: flow.rs
//! Purpose: Word wrap for a paragraph of differently styled words (a meaning in plain type followed by its example in italics), independent of how text is measured so it can be tested without the OS.
//! Contents: Piece — one word and its style; Placed — where a word goes; flow — wrap pieces into lines between a left indent and a right limit.
//! shortcut: a style is a bare `u8` index into the caller's font table, so a wrong index is a caller bug the type cannot catch; give it a style enum when a third style (beyond plain and italic) appears.

/// One word of a paragraph and the style (font index) it is drawn in.
pub struct Piece<'a> {
    pub text: &'a str,
    pub style: u8,
}

/// Where a word went: its piece number, left edge and line number.
#[derive(Debug, PartialEq)]
pub struct Placed {
    pub piece: usize,
    pub x: i32,
    pub line: usize,
}

/// The first line starts at `first_x`, later lines at `next_x`; nothing passes `right` unless a single word is wider than the line (it then sits alone).
/// Returns the words' places, the number of lines and where the last line ends.
pub fn flow(
    pieces: &[Piece],
    first_x: i32,
    next_x: i32,
    right: i32,
    measure: impl Fn(&str, u8) -> i32,
    space: impl Fn(u8) -> i32,
) -> (Vec<Placed>, usize, i32) {
    let (mut placed, mut line, mut x, mut line_start) = (Vec::with_capacity(pieces.len()), 0, first_x, first_x);
    for (i, piece) in pieces.iter().enumerate() {
        let width = measure(piece.text, piece.style);
        let gap = if x > line_start { space(piece.style) } else { 0 };
        if x > line_start && x + gap + width > right {
            line += 1;
            (x, line_start) = (next_x, next_x);
        } else {
            x += gap;
        }
        placed.push(Placed { piece: i, x, line });
        x += width;
    }
    (placed, line + 1, x)
}
