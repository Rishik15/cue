//! File: layout.rs
//! Purpose: Lay a dictionary card's text out on one left edge, like the macOS Look Up card: a large title with its hairline beside it, each word with its part of speech beside it, numbered meanings with their example in small italics on lines of its own below, all set small and close, as in the macOS card. Pure: text is measured through a callback so it can be tested.
//! Contents:
//! - Draw / Laid — what to draw where, the card's height and the full content height (the card scrolls when the content is taller)
//! - the style numbers
//! - lay_out — place everything for a width and scale

use crate::platform::flow::{flow, Piece};
use crate::platform::CardSection;

pub const BODY: u8 = 0;
pub const EXAMPLE: u8 = 1;
pub const LABEL: u8 = 2;
pub const WORD: u8 = 3;
pub const NUMBER: u8 = 4;
pub const TITLE: u8 = 5;
/// A part of speech: small and muted, so the headword above it stands out.
pub const POS: u8 = 6;

/// Sizes in dip.
pub const WIDTH: f32 = 300.0;
const MAX_HEIGHT: f32 = 320.0;
/// Meanings kept per part of speech: a word like "set" has hundreds, and nobody scrolls through them in a card; an ellipsis marks the cut. shortcut: 20, a "more" link to the full entry.
const MAX_ITEMS: usize = 20;
/// Meanings kept in all: the whole text is drawn once on a bitmap as tall as it is, so the total is bounded too. shortcut: 24, draw it in chunks as it scrolls.
const MAX_TOTAL: usize = 24;
const PAD_X: f32 = 20.0;
const PAD_TOP: f32 = 16.0;
const PAD_BOTTOM: f32 = 18.0;
const TITLE_LINE: f32 = 22.0;
const TITLE_GAP: f32 = 10.0;
/// The hairline runs through the middle of the title's capital letters.
const RULE_DROP: f32 = 9.9;
const WORD_LINE: f32 = 25.0;
const HEAD_GAP: f32 = 5.0;
const LINE: f32 = 18.0;
const ITEM_GAP: f32 = 8.0;
/// The space before a further part of speech, on top of the gap that already follows the last meaning.
const SECTION_GAP: f32 = 6.0;
const EXAMPLE_LINE: f32 = 16.0;
const EXAMPLE_GAP: f32 = 2.0;
const POS_LINE: f32 = 17.0;
/// The meanings start this far in from the left edge; their number stands on that edge, in line with the title and the word.
const TEXT_INDENT: f32 = 18.0;

/// One piece of text to draw: the text, its style and its position in px.
pub struct Draw {
    pub text: String,
    pub style: u8,
    pub x: i32,
    pub y: i32,
}

/// The laid-out card: the pieces to draw and the sizes of the card and its content.
pub struct Laid {
    pub draws: Vec<Draw>,
    /// The card's size in px: `height` is what shows, `content` the whole text (equal unless it scrolls).
    pub width: i32,
    pub height: i32,
    pub content: i32,
    /// Where the scrolling text starts: the title and its hairline above it stay put.
    pub body_top: i32,
    /// The hairline after the first title, which stays put: from x to x at y.
    pub rule: (i32, i32, i32),
}

/// Splits `text` into one piece per word, all in `style`.
fn words(text: &str, style: u8) -> Vec<Piece<'_>> {
    text.split_whitespace().map(|word| Piece { text: word, style }).collect()
}

/// Sizes in px for one scale, and the text measure, shared by the helpers.
struct Grid<'a> {
    scale: f32,
    pad: i32,
    indent: i32,
    right: i32,
    measure: &'a dyn Fn(&str, u8) -> i32,
}

impl<'a> Grid<'a> {
    /// Converts dip to px at this grid's scale.
    fn px(&self, dip: f32) -> i32 {
        (dip * self.scale).round() as i32
    }

    /// Wraps `pieces` from `x` at `y` with `line` between lines: the draws and the height used. A word wider than the line (a link, a long compound) would run past the padding: it is cut with an ellipsis.
    fn wrapped(&self, pieces: &[Piece], x: i32, y: i32, line: i32) -> (Vec<Draw>, i32) {
        let cut: Vec<String> = pieces.iter().map(|p| self.ellipsize(p.text, p.style, self.right - x)).collect();
        let pieces: Vec<Piece> = cut.iter().zip(pieces).map(|(text, p)| Piece { text, style: p.style }).collect();
        let (placed, lines, _) = flow(&pieces, x, x, self.right, self.measure, |style| (self.measure)(" ", style));
        let draws = placed.iter().map(|p| Draw {
            text: pieces[p.piece].text.into(),
            style: pieces[p.piece].style,
            x: p.x,
            y: y + p.line as i32 * line,
        });
        (draws.collect(), lines as i32 * line)
    }

    /// The title with its hairline beside it: the draw, and the rule from after the text to the right edge.
    fn title(&self, text: &str, y: i32) -> (Draw, (i32, i32, i32)) {
        let draw = Draw { text: text.into(), style: TITLE, x: self.pad, y };
        (draw, (self.pad + (self.measure)(text, TITLE) + self.px(9.0), self.right, y + self.px(RULE_DROP)))
    }

    /// Height of a section's head: the word with its part of speech, or the part of speech alone.
    fn head_height(&self, heading: Option<&String>) -> i32 {
        self.px(if heading.is_some() { WORD_LINE } else { POS_LINE })
    }

    /// The word (cut with an ellipsis when wider than the card) with its part of speech beside it when that fits, or the part of speech alone in small capitals. Sits on the headword's baseline: the 16.5 dip word's ascent is about 4.6 dip deeper than the label's.
    fn head(&self, heading: Option<&String>, label: &str, y: i32, draws: &mut Vec<Draw>) {
        let Some(word) = heading else {
            let label = self.ellipsize(label, POS, self.right - self.pad);
            draws.push(Draw { text: label, style: POS, x: self.pad, y });
            return;
        };
        let word = self.ellipsize(word, WORD, self.right - self.pad);
        let x = self.pad + (self.measure)(&word, WORD) + self.px(8.0);
        let fits = x + (self.measure)(label, POS) <= self.right;
        draws.push(Draw { text: word, style: WORD, x: self.pad, y });
        if fits {
            draws.push(Draw { text: label.into(), style: POS, x, y: y + self.px(4.6) });
        }
    }

    /// `text`, or its start and an ellipsis when it is wider than `max`.
    fn ellipsize(&self, text: &str, style: u8, max: i32) -> String {
        if (self.measure)(text, style) <= max {
            return text.into();
        }
        let mut kept: Vec<char> = text.chars().collect();
        while kept.pop().is_some() {
            let cut: String = kept.iter().chain(&['…']).collect();
            if (self.measure)(&cut, style) <= max {
                return cut;
            }
        }
        "…".into()
    }

    /// Builds the grid of sizes for `scale` and the text `measure` (text and style to width in px).
    fn new(scale: f32, measure: &'a dyn Fn(&str, u8) -> i32) -> Self {
        let px = |dip: f32| (dip * scale).round() as i32;
        let pad = px(PAD_X);
        Grid { scale, pad, indent: pad + px(TEXT_INDENT), right: px(WIDTH) - pad, measure }
    }
}

/// One meaning: its number centred in the margin (when there is more than one), the meaning, then its example in small italics on lines of its own. Returns the draws and the height used. Down by the difference of the two fonts' ascents (13 dip body, 12 dip number), so the digit sits on the first line's baseline.
fn item(grid: &Grid, number: Option<usize>, (gloss, example): &(String, Option<String>), y: i32) -> (Vec<Draw>, i32) {
    let (mut draws, mut height) = grid.wrapped(&words(gloss, BODY), grid.indent, y, grid.px(LINE));
    if let Some(n) = number {
        let text = n.to_string();
        let gutter = grid.indent - grid.pad;
        let x = grid.pad + (gutter - (grid.measure)(&text, NUMBER)) / 2;
        draws.push(Draw { text, style: NUMBER, x, y: y + grid.px(1.0) });
    }
    if let Some(example) = example {
        let (lines, taller) = grid.wrapped(&words(example, EXAMPLE), grid.indent, y + height + grid.px(EXAMPLE_GAP), grid.px(EXAMPLE_LINE));
        draws.extend(lines);
        height += grid.px(EXAMPLE_GAP) + taller;
    }
    (draws, height)
}

/// The dictionary part: each section's head and its meanings; the cap on meanings may cut it short, which an ellipsis marks. Returns where it ended.
fn meanings(grid: &Grid, sections: &[CardSection], mut y: i32, draws: &mut Vec<Draw>) -> i32 {
    let mut left = MAX_TOTAL;
    for (at, section) in sections.iter().enumerate() {
        y += if at > 0 { grid.px(SECTION_GAP) } else { 0 };
        grid.head(section.heading.as_ref(), &section.label, y, draws);
        y += grid.head_height(section.heading.as_ref()) + grid.px(HEAD_GAP);
        let shown = section.items.len().min(MAX_ITEMS).min(left);
        left -= shown;
        for (n, entry) in section.items.iter().take(shown).enumerate() {
            let (lines, height) = item(grid, (section.items.len() > 1).then_some(n + 1), entry, y);
            draws.extend(lines);
            y += height + grid.px(ITEM_GAP);
        }
        if section.items.len() > shown || (left == 0 && at + 1 < sections.len()) {
            draws.push(Draw { text: "…".into(), style: LABEL, x: grid.indent, y });
            y += grid.px(LINE);
        }
        if left == 0 {
            break;
        }
    }
    y
}

/// Lays the card out. Input: the title and sections, the scale, `room` (the tallest the card may be, in pixels, so it fits a small work area; the text scrolls past it) and the text measure. Return: the drawing, the card size and the content height.
pub fn lay_out(title: &str, sections: &[CardSection], scale: f32, room: i32, measure: &dyn Fn(&str, u8) -> i32) -> Laid {
    let grid = Grid::new(scale, measure);
    let (first, rule) = grid.title(title, grid.px(PAD_TOP));
    let mut draws = vec![first];
    let body_top = grid.px(PAD_TOP) + grid.px(TITLE_LINE) + grid.px(TITLE_GAP);
    let y = meanings(&grid, sections, body_top, &mut draws);
    let content = y + grid.px(PAD_BOTTOM);
    Laid { draws, width: grid.px(WIDTH), height: content.min(grid.px(MAX_HEIGHT).min(room)), content, body_top, rule }
}
