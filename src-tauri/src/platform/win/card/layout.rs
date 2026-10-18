//! Purpose: Lay a dictionary card's text out: the "Dictionary" title with its hairline, each word with its part of speech beside it, numbered meanings that wrap with their example following in italics, and the synonyms. Pure: text is measured through a callback so it can be tested.
//! Contents: Draw / Laid — what to draw where; the style numbers; lay_out — place everything for a width and scale, dropping what does not fit under the height limit.

use crate::platform::flow::{flow, Piece};
use crate::platform::CardSection;

pub const BODY: u8 = 0;
pub const EXAMPLE: u8 = 1;
pub const LABEL: u8 = 2;
pub const WORD: u8 = 3;
pub const NUMBER: u8 = 4;
pub const TITLE: u8 = 5;

/// Sizes in dip.
pub const WIDTH: f32 = 360.0;
const MAX_HEIGHT: f32 = 400.0;
const PAD_X: f32 = 20.0;
const PAD_TOP: f32 = 15.0;
const PAD_BOTTOM: f32 = 16.0;
const LINE: f32 = 18.0;
const WORD_LINE: f32 = 26.0;
const TITLE_LINE: f32 = 22.0;
const NUMBER_COLUMN: f32 = 22.0;

pub struct Draw {
    pub text: String,
    pub style: u8,
    pub x: i32,
    pub y: i32,
}

pub struct Laid {
    pub draws: Vec<Draw>,
    /// The card's size in px.
    pub width: i32,
    pub height: i32,
    /// The hairline after the title: from x to x at y.
    pub rule: (i32, i32, i32),
}

/// A paragraph: the meaning in plain type, then its example in italics.
fn paragraph<'a>(first: &'a str, second: Option<&'a str>) -> Vec<Piece<'a>> {
    let words = |text: &'a str, style: u8| text.split_whitespace().map(move |word| Piece { text: word, style });
    words(first, BODY).chain(second.into_iter().flat_map(|text| words(text, EXAMPLE))).collect()
}

/// Sizes in px for one scale, and the text measure, shared by the helpers.
struct Grid<'a> {
    scale: f32,
    pad: i32,
    line: i32,
    indent: i32,
    right: i32,
    measure: &'a dyn Fn(&str, u8) -> i32,
}

impl<'a> Grid<'a> {
    fn px(&self, dip: f32) -> i32 {
        (dip * self.scale).round() as i32
    }

    /// Wraps `pieces` under the number column starting at `y`; None when they would pass `limit`, else the draws and the lines used.
    fn wrapped(&self, pieces: &[Piece], y: i32, limit: i32) -> Option<(Vec<Draw>, i32)> {
        let (placed, lines, _) = flow(pieces, self.indent, self.indent, self.right, self.measure, |style| (self.measure)(" ", style));
        let height = lines as i32 * self.line;
        let draws = placed.iter().map(|p| Draw {
            text: pieces[p.piece].text.into(),
            style: pieces[p.piece].style,
            x: p.x,
            y: y + p.line as i32 * self.line,
        });
        (y + height <= limit).then(|| (draws.collect(), height))
    }

    /// The "Synonyms" line wrapped under the number column; nothing (zero height) when there are none, None when it would pass `limit`.
    fn synonyms(&self, synonyms: &[String], y: i32, limit: i32) -> Option<(Vec<Draw>, i32)> {
        if synonyms.is_empty() {
            return Some((Vec::new(), 0));
        }
        let words = comma_separated(synonyms);
        let label = Piece { text: "Synonyms", style: LABEL };
        let pieces: Vec<Piece> = std::iter::once(label).chain(words.iter().map(|text| Piece { text, style: EXAMPLE })).collect();
        self.wrapped(&pieces, y, limit)
    }

    /// Height of a section's head: the large word, or the part of speech alone.
    fn head_height(&self, section: &CardSection) -> i32 {
        if section.heading.is_some() {
            self.px(WORD_LINE)
        } else {
            self.line
        }
    }

    /// The word (cut with an ellipsis when wider than the card) with its part of speech beside it when that fits, or the part of speech alone.
    fn head(&self, section: &CardSection, y: i32, draws: &mut Vec<Draw>) {
        let Some(word) = &section.heading else {
            draws.push(Draw { text: section.label.clone(), style: LABEL, x: self.pad, y });
            return;
        };
        let word = self.ellipsize(word, WORD, self.right - self.pad);
        let x = self.pad + (self.measure)(&word, WORD) + self.px(9.0);
        let fits = x + (self.measure)(&section.label, LABEL) <= self.right;
        draws.push(Draw { text: word, style: WORD, x: self.pad, y });
        if fits {
            draws.push(Draw { text: section.label.clone(), style: LABEL, x, y: y + self.px(WORD_LINE) - self.line - self.px(2.0) });
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
        Grid { scale, pad, line: px(LINE), indent: pad + px(NUMBER_COLUMN), right: px(WIDTH) - pad, measure }
    }
}

/// The synonyms as flow pieces' text: every one but the last followed by a comma.
fn comma_separated(synonyms: &[String]) -> Vec<String> {
    let last = synonyms.len().saturating_sub(1);
    synonyms.iter().enumerate().map(|(i, s)| if i < last { format!("{s},") } else { s.clone() }).collect()
}

pub fn lay_out(sections: &[CardSection], scale: f32, measure: &dyn Fn(&str, u8) -> i32) -> Laid {
    let grid = Grid::new(scale, measure);
    // What would pass `limit` is dropped; room is kept under it for the ellipsis line.
    let limit = grid.px(MAX_HEIGHT) - grid.px(PAD_BOTTOM) - grid.line;
    let mut draws = vec![Draw { text: "Dictionary".into(), style: TITLE, x: grid.pad, y: grid.px(PAD_TOP) }];
    let rule = (grid.pad + measure("Dictionary", TITLE) + grid.px(10.0), grid.right, grid.px(PAD_TOP) + grid.px(TITLE_LINE) / 2);
    let mut y = grid.px(PAD_TOP) + grid.px(TITLE_LINE) + grid.px(8.0);
    let mut cut = false;
    'sections: for (at, section) in sections.iter().enumerate() {
        y += if at > 0 { grid.px(10.0) } else { 0 };
        if y + grid.head_height(section) > limit {
            cut = true;
            break;
        }
        grid.head(section, y, &mut draws);
        y += grid.head_height(section) + grid.px(4.0);
        for (n, (gloss, example)) in section.items.iter().enumerate() {
            let Some((lines, height)) = grid.wrapped(&paragraph(gloss, example.as_deref()), y, limit) else {
                cut = true;
                break 'sections;
            };
            draws.push(Draw { text: (n + 1).to_string(), style: NUMBER, x: grid.pad, y: y + grid.px(1.0) });
            draws.extend(lines);
            y += height + grid.px(4.0);
        }
        let Some((lines, height)) = grid.synonyms(&section.synonyms, y, limit) else {
            cut = true;
            break;
        };
        draws.extend(lines);
        y += height;
    }
    if cut {
        draws.push(Draw { text: "…".into(), style: LABEL, x: grid.indent, y });
        y += grid.line;
    }
    Laid { draws, width: grid.px(WIDTH), height: y + grid.px(PAD_BOTTOM), rule }
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
        assert_eq!(numbers[1].y - numbers[0].y, 18 + 4);
        assert_eq!(laid.width, 360);
    }

    #[test]
    fn long_text_wraps_inside_the_card() {
        let text = "word ".repeat(60);
        let laid = lay_out(&[section(&[(&text, Some("an example sentence"))])], 1.0, &measure);
        assert!(laid.draws.iter().all(|d| d.x + measure(&d.text, d.style) <= 360 - 20), "nothing passes the right padding");
        assert!(laid.draws.iter().any(|d| d.style == EXAMPLE));
    }

    #[test]
    fn what_does_not_fit_is_dropped_with_an_ellipsis() {
        let many: Vec<(&str, Option<&str>)> = (0..40).map(|_| ("a meaning of the word", None)).collect();
        let laid = lay_out(&[section(&many)], 1.0, &measure);
        assert!(laid.height <= 400, "the card stays under its height limit");
        assert_eq!(laid.draws.last().map(|d| d.text.as_str()), Some("…"));
    }

    #[test]
    fn a_headword_wider_than_the_card_is_cut_and_the_label_dropped() {
        let mut long = section(&[("to move", None)]);
        long.heading = Some("w".repeat(80));
        let laid = lay_out(&[long], 1.0, &measure);
        let word = laid.draws.iter().find(|d| d.style == WORD).unwrap();
        assert!(word.text.ends_with('…') && word.x + measure(&word.text, WORD) <= 360 - 20);
        assert!(laid.draws.iter().all(|d| d.style != LABEL), "no room beside the word for the part of speech");
    }

    #[test]
    fn sections_past_the_height_limit_are_cut_at_their_head() {
        let sections: Vec<_> = (0..30).map(|_| section(&[("a meaning", None)])).collect();
        let laid = lay_out(&sections, 1.0, &measure);
        assert!(laid.height <= 400);
        assert_eq!(laid.draws.last().map(|d| d.text.as_str()), Some("…"));
    }

    #[test]
    fn scale_grows_everything() {
        let one = lay_out(&[section(&[("to move", None)])], 1.0, &measure);
        let two = lay_out(&[section(&[("to move", None)])], 2.0, &|t, s| measure(t, s) * 2);
        assert_eq!(two.width, one.width * 2);
        assert_eq!(two.height, one.height * 2);
    }
}
