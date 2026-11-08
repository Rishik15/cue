//! File: reflow.rs
//! Purpose: Turns the lines the text reader returns into paragraphs: a paragraph wrapped on screen comes back as one line of text, not one line per screen row.
//! Contents: is_item — whether a line starts a list item; Line — a read line with its position; reflow — lines to text (blank line between paragraphs, list items and indents keep their own line).

/// A read line with its position on the picture.
pub struct Line {
    /// The line's text.
    pub text: String,
    /// Left edge of its words.
    pub left: f32,
    /// Top edge of its words.
    pub top: f32,
    /// Bottom edge of its words.
    pub bottom: f32,
}

/// A gap taller than this share of a line (word boxes are tight, so normal line spacing leaves 0.5 to 0.8) starts a new paragraph.
const PARAGRAPH_GAP: f32 = 1.0;

/// Whether the line starts a list item (a bullet or a number).
fn is_item(text: &str) -> bool {
    let t = text.trim_start();
    let digits = t.chars().take_while(char::is_ascii_digit).count();
    t.starts_with(['-', '*', '•', '–']) || (digits > 0 && t[digits..].starts_with(['.', ')']))
}

/// The lines as text: a gap starts a paragraph, an item or an indent starts a line, anything else joins with a space. With `join_hyphens` a word the screen broke with a hyphen at the line end is joined (a guess: `well-` and `known` is a real hyphen); without it every character the reader gave is kept.
pub fn reflow(lines: &[Line], join_hyphens: bool) -> String {
    let mut out = String::new();
    let mut prev: Option<&Line> = None;
    for line in lines {
        let Some(p) = prev.replace(line) else {
            out.push_str(&line.text);
            continue;
        };
        let height = (p.bottom - p.top).max(1.0);
        if line.top - p.bottom > PARAGRAPH_GAP * height {
            out.push_str("\n\n");
        } else if is_item(&line.text) || (line.left - p.left).abs() > height {
            out.push('\n');
        } else if join_hyphens && out.ends_with('-') && line.text.starts_with(char::is_lowercase) {
            out.pop();
        } else {
            out.push(' ');
        }
        out.push_str(&line.text);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{reflow, Line};

    fn line(text: &str, top: f32) -> Line {
        Line { text: text.into(), left: 0.0, top, bottom: top + 10.0 }
    }

    /// A real hyphen survives in the raw text; only the repaired reading joins a word broken at the line end.
    #[test]
    fn a_line_end_hyphen_is_joined_only_when_asked() {
        let lines = [line("a well-", 0.0), line("known fact", 12.0)];
        assert_eq!(reflow(&lines, false), "a well- known fact");
        assert_eq!(reflow(&lines, true), "a wellknown fact");
    }

    /// A gap taller than a line starts a paragraph either way.
    #[test]
    fn a_tall_gap_starts_a_paragraph() {
        let lines = [line("one", 0.0), line("two", 40.0)];
        assert_eq!(reflow(&lines, false), "one\n\ntwo");
    }
}
