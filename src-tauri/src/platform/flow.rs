//! Purpose: Word wrap for a paragraph of differently styled words (a meaning in plain type followed by its example in italics), independent of how text is measured so it can be tested without the OS.
//! Contents: Piece — one word and its style; Placed — where a word goes; flow — wrap pieces into lines between a left indent and a right limit.

pub struct Piece<'a> {
    pub text: &'a str,
    pub style: u8,
}

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

#[cfg(test)]
mod tests {
    use super::*;

    fn words(text: &str) -> Vec<Piece<'_>> {
        text.split(' ').map(|text| Piece { text, style: 0 }).collect()
    }

    /// Every character is 10 wide, a space 10.
    fn run(text: &str, first_x: i32, next_x: i32, right: i32) -> (Vec<Placed>, usize, i32) {
        flow(&words(text), first_x, next_x, right, |t, _| t.len() as i32 * 10, |_| 10)
    }

    #[test]
    fn words_stay_on_one_line_while_they_fit() {
        let (placed, lines, end) = run("ab cd", 0, 0, 100);
        assert_eq!((placed.iter().map(|p| p.x).collect::<Vec<_>>(), lines, end), (vec![0, 30], 1, 50));
    }

    #[test]
    fn a_word_that_does_not_fit_starts_the_next_line_at_the_indent() {
        let (placed, lines, _) = run("aaaa bbbb cccc", 20, 5, 120);
        assert_eq!(placed.iter().map(|p| (p.x, p.line)).collect::<Vec<_>>(), vec![(20, 0), (70, 0), (5, 1)]);
        assert_eq!(lines, 2);
    }

    #[test]
    fn a_word_wider_than_the_line_sits_alone() {
        let (placed, lines, _) = run("a bbbbbbbbbbbbbbbbbbbb c", 0, 0, 100);
        assert_eq!(placed.iter().map(|p| p.line).collect::<Vec<_>>(), vec![0, 1, 2]);
        assert_eq!(lines, 3);
    }

    #[test]
    fn nothing_to_place_is_one_empty_line() {
        let (placed, lines, end) = flow(&[], 7, 7, 100, |_, _| 0, |_| 0);
        assert_eq!((placed.len(), lines, end), (0, 1, 7));
    }
}
