//! Purpose: Turns the lines the text reader returns into paragraphs: a paragraph wrapped on screen comes back as one line of text, not one line per screen row.
//! Contents: Line — a read line with its position; reflow — lines to text (blank line between paragraphs, list items and indents keep their own line).

pub struct Line {
    pub text: String,
    pub left: f32,
    pub top: f32,
    pub bottom: f32,
}

/// A gap taller than this share of a line (word boxes are tight, so normal line spacing leaves 0.5 to 0.8) starts a new paragraph.
const PARAGRAPH_GAP: f32 = 1.0;

fn is_item(text: &str) -> bool {
    let t = text.trim_start();
    let digits = t.chars().take_while(char::is_ascii_digit).count();
    t.starts_with(['-', '*', '•', '–']) || (digits > 0 && t[digits..].starts_with(['.', ')']))
}

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
            out.pop(); // a word the screen broke at the line end
        } else {
            out.push(' ');
        }
        out.push_str(&line.text);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(text: &str, left: f32, top: f32) -> Line {
        Line { text: text.into(), left, top, bottom: top + 16.0 }
    }

    #[test]
    fn a_wrapped_paragraph_becomes_one_line() {
        let lines = [line("We will compare the", 10.0, 0.0), line("versions on the same", 10.0, 20.0), line("workload.", 10.0, 40.0)];
        assert_eq!(reflow(&lines), "We will compare the versions on the same workload.");
    }

    #[test]
    fn a_big_gap_starts_a_new_paragraph() {
        assert_eq!(reflow(&[line("One.", 10.0, 0.0), line("Two.", 10.0, 50.0)]), "One.\n\nTwo.");
    }

    #[test]
    fn list_items_keep_their_own_lines() {
        let lines = [line("- first", 10.0, 0.0), line("- second", 10.0, 20.0), line("3. third", 10.0, 40.0)];
        assert_eq!(reflow(&lines), "- first\n- second\n3. third");
    }

    #[test]
    fn a_word_broken_at_the_line_end_is_joined() {
        assert_eq!(reflow(&[line("paral-", 10.0, 0.0), line("lel runs", 10.0, 20.0)]), "parallel runs");
    }
}
