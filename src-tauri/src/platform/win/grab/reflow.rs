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

/// The lines as text: a gap starts a paragraph, an item or an indent starts a line, a word the screen broke with a hyphen at the line end is joined, anything else joins with a space.
pub fn reflow(lines: &[Line]) -> String {
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
        } else if out.ends_with('-') && line.text.starts_with(char::is_lowercase) {
            out.pop();
        } else {
            out.push(' ');
        }
        out.push_str(&line.text);
    }
    out
}
