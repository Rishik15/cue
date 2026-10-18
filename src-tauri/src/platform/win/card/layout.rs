//! Purpose: Lay a dictionary card's text out on one left edge: the "Dictionary" title with its hairline, each word with its part of speech beside it, numbered meanings with their example on a line of its own below, a small-caps label for each further part of speech, and the synonyms. Pure: text is measured through a callback so it can be tested.
//! Contents: Draw / Laid — what to draw where, the card's height and the full content height (the card scrolls when the content is taller); the style numbers; lay_out — place everything for a width and scale.

use crate::platform::flow::{flow, Piece};
use crate::platform::CardSection;

pub const BODY: u8 = 0;
pub const EXAMPLE: u8 = 1;
pub const LABEL: u8 = 2;
pub const WORD: u8 = 3;
pub const NUMBER: u8 = 4;
pub const TITLE: u8 = 5;
/// The small capitals over a further part of speech and the synonyms.
pub const SECTION: u8 = 6;

/// Sizes in dip.
pub const WIDTH: f32 = 270.0;
const MAX_HEIGHT: f32 = 320.0;
/// Meanings kept per part of speech: a word like "set" has hundreds, and nobody scrolls through them in a card; an ellipsis marks the cut. ponytail: 20, a "more" link to the full entry.
const MAX_ITEMS: usize = 20;
/// Meanings kept in all: the whole text is drawn once on a bitmap as tall as it is, so the total is bounded too. ponytail: 24, draw it in chunks as it scrolls.
const MAX_TOTAL: usize = 24;
const PAD_X: f32 = 22.0;
const PAD_TOP: f32 = 16.0;
const PAD_BOTTOM: f32 = 18.0;
const TITLE_LINE: f32 = 20.0;
const TITLE_GAP: f32 = 14.0;
const WORD_LINE: f32 = 26.0;
const HEAD_GAP: f32 = 8.0;
const LINE: f32 = 20.0;
const EXAMPLE_LINE: f32 = 19.0;
const EXAMPLE_GAP: f32 = 1.0;
const ITEM_GAP: f32 = 10.0;
const SECTION_GAP: f32 = 18.0;
const SECTION_LINE: f32 = 16.0;
/// The meanings start this far in from the left edge; their number stands on that edge, in line with the title and the word.
const TEXT_INDENT: f32 = 20.0;

pub struct Draw {
    pub text: String,
    pub style: u8,
    pub x: i32,
    pub y: i32,
}

pub struct Laid {
    pub draws: Vec<Draw>,
    /// The card's size in px: `height` is what shows, `content` the whole text (equal unless it scrolls).
    pub width: i32,
    pub height: i32,
    pub content: i32,
    /// Where the scrolling text starts: the title and its hairline above it stay put.
    pub body_top: i32,
    /// The hairline after the title: from x to x at y.
    pub rule: (i32, i32, i32),
}

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
    fn px(&self, dip: f32) -> i32 {
        (dip * self.scale).round() as i32
    }

    /// Wraps `pieces` from `x` at `y` with `line` between lines: the draws and the height used.
    fn wrapped(&self, pieces: &[Piece], x: i32, y: i32, line: i32) -> (Vec<Draw>, i32) {
        // A word wider than the line (a link, a long compound) would run past the padding: it is cut with an ellipsis.
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

    /// The "SYNONYMS" label on a line of its own and the words under it; nothing (zero height) when there are none.
    fn synonyms(&self, synonyms: &[String], y: i32) -> (Vec<Draw>, i32) {
        if synonyms.is_empty() {
            return (Vec::new(), 0);
        }
        let label = Draw { text: "SYNONYMS".into(), style: SECTION, x: self.pad, y: y + self.px(6.0) };
        let list = comma_separated(synonyms);
        let pieces: Vec<Piece> = list.iter().map(|text| Piece { text, style: LABEL }).collect();
        let under = y + self.px(6.0) + self.px(SECTION_LINE);
        let (mut draws, height) = self.wrapped(&pieces, self.pad, under, self.px(18.0));
        draws.insert(0, label);
        (draws, height + self.px(6.0) + self.px(SECTION_LINE))
    }

    /// Height of a section's head: the large word, or the small-caps part of speech alone.
    fn head_height(&self, section: &CardSection) -> i32 {
        self.px(if section.heading.is_some() { WORD_LINE } else { SECTION_LINE })
    }

    /// The word (cut with an ellipsis when wider than the card) with its part of speech beside it when that fits, or the part of speech alone in small capitals.
    fn head(&self, section: &CardSection, y: i32, draws: &mut Vec<Draw>) {
        let Some(word) = &section.heading else {
            let label = self.ellipsize(&section.label.to_uppercase(), SECTION, self.right - self.pad);
            draws.push(Draw { text: label, style: SECTION, x: self.pad, y });
            return;
        };
        let word = self.ellipsize(word, WORD, self.right - self.pad);
        let x = self.pad + (self.measure)(&word, WORD) + self.px(9.0);
        let fits = x + (self.measure)(&section.label, LABEL) <= self.right;
        draws.push(Draw { text: word, style: WORD, x: self.pad, y });
        if fits {
            // Sits on the headword's baseline: the 19 dip word's ascent is about 6 dip deeper than the small label's.
            draws.push(Draw { text: section.label.clone(), style: LABEL, x, y: y + self.px(6.3) });
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

    fn new(scale: f32, measure: &'a dyn Fn(&str, u8) -> i32) -> Self {
        let px = |dip: f32| (dip * scale).round() as i32;
        let pad = px(PAD_X);
        Grid { scale, pad, indent: pad + px(TEXT_INDENT), right: px(WIDTH) - pad, measure }
    }
}

/// The synonyms as flow pieces' text: every one but the last followed by a comma.
fn comma_separated(synonyms: &[String]) -> Vec<String> {
    let last = synonyms.len().saturating_sub(1);
    synonyms.iter().enumerate().map(|(i, s)| if i < last { format!("{s},") } else { s.clone() }).collect()
}

/// One numbered meaning: its number hanging in the margin, the meaning, then its example on lines of its own. Returns the draws and the height used.
fn item(grid: &Grid, n: usize, (gloss, example): &(String, Option<String>), y: i32) -> (Vec<Draw>, i32) {
    let (mut draws, mut height) = grid.wrapped(&words(gloss, BODY), grid.indent, y, grid.px(LINE));
    let number = (n + 1).to_string();
    // Down by the difference of the two fonts' ascents (14 dip body, 12.5 dip number), so the digit sits on the first line's baseline.
    draws.push(Draw { text: number, style: NUMBER, x: grid.pad, y: y + grid.px(1.5) });
    if let Some(example) = example {
        let (lines, taller) = grid.wrapped(&words(example, EXAMPLE), grid.indent, y + height + grid.px(EXAMPLE_GAP), grid.px(EXAMPLE_LINE));
        draws.extend(lines);
        height += grid.px(EXAMPLE_GAP) + taller;
    }
    (draws, height)
}

pub fn lay_out(sections: &[CardSection], scale: f32, measure: &dyn Fn(&str, u8) -> i32) -> Laid {
    let grid = Grid::new(scale, measure);
    let mut draws = vec![Draw { text: "Dictionary".into(), style: TITLE, x: grid.pad, y: grid.px(PAD_TOP) }];
    let rule = (grid.pad + measure("Dictionary", TITLE) + grid.px(10.0), grid.right, grid.px(PAD_TOP) + grid.px(7.8)); // the middle of the capital letters of the title
    let body_top = grid.px(PAD_TOP) + grid.px(TITLE_LINE) + grid.px(TITLE_GAP);
    let mut y = body_top;
    let mut left = MAX_TOTAL;
    for (at, section) in sections.iter().enumerate() {
        y += if at > 0 { grid.px(SECTION_GAP) } else { 0 };
        grid.head(section, y, &mut draws);
        y += grid.head_height(section) + grid.px(HEAD_GAP);
        let shown = section.items.len().min(MAX_ITEMS).min(left);
        left -= shown;
        for (n, entry) in section.items.iter().take(shown).enumerate() {
            let (lines, height) = item(&grid, n, entry, y);
            draws.extend(lines);
            y += height + grid.px(ITEM_GAP);
        }
        if section.items.len() > shown || (left == 0 && at + 1 < sections.len()) {
            draws.push(Draw { text: "…".into(), style: LABEL, x: grid.indent, y });
            y += grid.px(LINE);
        }
        let (lines, height) = grid.synonyms(&section.synonyms, y);
        draws.extend(lines);
        y += height;
        if left == 0 {
            break;
        }
    }
    let content = y + grid.px(PAD_BOTTOM);
    Laid { draws, width: grid.px(WIDTH), height: content.min(grid.px(MAX_HEIGHT)), content, body_top, rule }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn section(items: &[(&str, Option<&str>)]) -> CardSection {
        CardSection {
            heading: Some("go".into()),
            label: "verb".into(),
            items: items.iter().map(|(g, e)| (g.to_string(), e.map(str::to_string))).collect(),
            synonyms: vec![],
        }
    }

    /// Each character 6 px wide, whatever the style.
    fn measure(text: &str, _: u8) -> i32 {
        text.chars().count() as i32 * 6
    }

    #[test]
    fn a_short_entry_is_one_line_per_meaning() {
        let laid = lay_out(&[section(&[("to move", None), ("to leave", None)])], 1.0, &measure);
        let numbers: Vec<_> = laid.draws.iter().filter(|d| d.style == NUMBER).collect();
        assert_eq!(numbers.len(), 2);
        assert_eq!(numbers[1].y - numbers[0].y, 20 + 10);
        assert_eq!(laid.width, 270);
    }

    #[test]
    fn long_text_wraps_inside_the_card() {
        let text = "word ".repeat(60);
        let laid = lay_out(&[section(&[(&text, Some("an example sentence"))])], 1.0, &measure);
        assert!(laid.draws.iter().all(|d| d.x + measure(&d.text, d.style) <= 270 - 22), "nothing passes the right padding");
        assert!(laid.draws.iter().any(|d| d.style == EXAMPLE));
    }

    #[test]
    fn a_long_entry_keeps_its_height_limit_and_scrolls() {
        let many: Vec<(&str, Option<&str>)> = (0..25).map(|_| ("a meaning of the word", None)).collect();
        let laid = lay_out(&[section(&many)], 1.0, &measure);
        assert_eq!(laid.height, 320, "the card stays under its height limit");
        assert!(laid.content > laid.height, "the rest is reached by scrolling");
    }

    #[test]
    fn a_short_entry_does_not_scroll() {
        let laid = lay_out(&[section(&[("to move", None)])], 1.0, &measure);
        assert_eq!(laid.content, laid.height);
    }

    #[test]
    fn meanings_past_the_cap_are_not_laid_out() {
        let many: Vec<(&str, Option<&str>)> = (0..100).map(|_| ("a meaning", None)).collect();
        let laid = lay_out(&[section(&many)], 1.0, &measure);
        assert_eq!(laid.draws.iter().filter(|d| d.style == NUMBER).count(), MAX_ITEMS);
        assert!(laid.draws.iter().any(|d| d.text == "…"), "the cut is marked");
    }

    #[test]
    fn the_total_of_meanings_is_bounded_across_parts_of_speech() {
        let many: Vec<(&str, Option<&str>)> = (0..20).map(|_| ("a meaning", None)).collect();
        let laid = lay_out(&[section(&many), section(&many), section(&many)], 1.0, &measure);
        assert_eq!(laid.draws.iter().filter(|d| d.style == NUMBER).count(), MAX_TOTAL);
        assert!(laid.draws.iter().any(|d| d.text == "…"), "the cut is marked");
    }

    #[test]
    fn a_word_wider_than_the_line_is_cut_inside_the_padding() {
        let long = "w".repeat(80);
        let laid = lay_out(&[section(&[(&long, Some(&long))])], 1.0, &measure);
        assert!(laid.draws.iter().all(|d| d.x + measure(&d.text, d.style) <= 270 - 22), "nothing passes the right padding");
        assert!(laid.draws.iter().any(|d| d.text.ends_with('…')));
    }

    #[test]
    fn a_headword_wider_than_the_card_is_cut_and_the_label_dropped() {
        let mut long = section(&[("to move", None)]);
        long.heading = Some("w".repeat(80));
        let laid = lay_out(&[long], 1.0, &measure);
        let word = laid.draws.iter().find(|d| d.style == WORD).unwrap();
        assert!(word.text.ends_with('…') && word.x + measure(&word.text, WORD) <= 270 - 22);
        assert!(laid.draws.iter().all(|d| d.style != LABEL), "no room beside the word for the part of speech");
    }

    #[test]
    fn scale_grows_everything() {
        let one = lay_out(&[section(&[("to move", None)])], 1.0, &measure);
        let two = lay_out(&[section(&[("to move", None)])], 2.0, &|t, s| measure(t, s) * 2);
        assert_eq!(two.width, one.width * 2);
        assert_eq!(two.height, one.height * 2);
    }
}
