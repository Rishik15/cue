//! Purpose: Split a text into the units the model works on (sentences or paragraphs) and put the answers back, keeping every space, line break and unit that is not worth sending.
//! Contents: Seg — a unit to send or text to keep as it is; split — cut a text into segments; join — rebuild the text from segments and answers.
//! A small model drifts on long input (it adds or drops sentences, or answers the text), so corrections go one sentence at a time. Joining the untouched segments gives back the exact input.

use crate::actions::Unit;

#[derive(Debug, PartialEq, Eq)]
pub enum Seg<'a> {
    /// A unit with letters in it: sent to the model.
    Unit(&'a str),
    /// Whitespace, or a piece with no letters (a number, a lone symbol): kept as it is.
    Keep(&'a str),
}

/// Closing quotes and brackets that stay with the sentence they close.
const CLOSERS: [char; 6] = ['"', '\'', ')', ']', '\u{201d}', '\u{2019}'];

/// A blank line (another line break, with only spaces between) follows the line break at the start of `rest`.
fn blank_line_follows(rest: &str) -> bool {
    rest[1..].trim_start_matches([' ', '\t', '\r']).starts_with('\n')
}

/// The end (exclusive) of the unit that starts at `from`: after `.`, `!` or `?` (and closing quotes) when whitespace follows, or at a line break;
/// for paragraphs, at a blank line.
fn unit_end(text: &str, from: usize, unit: Unit) -> usize {
    for (i, c) in text[from..].char_indices() {
        let at = from + i;
        match unit {
            Unit::Paragraph if c == '\n' && blank_line_follows(&text[at..]) => return at,
            Unit::Sentence if c == '\n' => return at,
            Unit::Sentence if matches!(c, '.' | '!' | '?') => {
                let rest = text[at + 1..].trim_start_matches(CLOSERS);
                if rest.is_empty() || rest.starts_with(char::is_whitespace) {
                    return text.len() - rest.len();
                }
            }
            _ => {}
        }
    }
    text.len()
}

pub fn split(text: &str, unit: Unit) -> Vec<Seg<'_>> {
    let mut segs = Vec::new();
    let mut pos = 0;
    while pos < text.len() {
        let rest = &text[pos..];
        let blank = rest.len() - rest.trim_start().len();
        if blank > 0 {
            segs.push(Seg::Keep(&rest[..blank]));
            pos += blank;
            continue;
        }
        let end = unit_end(text, pos, unit);
        let piece = text[pos..end].trim_end();
        let (body, trail) = (piece, &text[pos + piece.len()..end]);
        segs.push(if body.chars().any(char::is_alphabetic) { Seg::Unit(body) } else { Seg::Keep(body) });
        if !trail.is_empty() {
            segs.push(Seg::Keep(trail));
        }
        pos = end;
    }
    segs
}

/// Rebuilds the text: each `Unit` is replaced by the next answer, everything else is copied.
pub fn join(segs: &[Seg], answers: &[String]) -> String {
    let mut answers = answers.iter();
    segs.iter()
        .map(|seg| match seg {
            Seg::Unit(original) => answers.next().map_or(*original, String::as_str),
            Seg::Keep(text) => text,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn units(text: &str, unit: Unit) -> Vec<&str> {
        split(text, unit).into_iter().filter_map(|s| if let Seg::Unit(u) = s { Some(u) } else { None }).collect()
    }

    fn rebuilt(text: &str, unit: Unit) -> String {
        let segs = split(text, unit);
        let same: Vec<String> = segs.iter().filter_map(|s| if let Seg::Unit(u) = s { Some(u.to_string()) } else { None }).collect();
        join(&segs, &same)
    }

    #[test]
    fn sentences_split_at_terminal_punctuation_and_line_breaks() {
        assert_eq!(units("i has a apple. she dont like it! ok?", Unit::Sentence), ["i has a apple.", "she dont like it!", "ok?"]);
        assert_eq!(units("first line\nsecond line", Unit::Sentence), ["first line", "second line"]);
        assert_eq!(units("He said \"go.\" Then left.", Unit::Sentence), ["He said \"go.\"", "Then left."]);
    }

    #[test]
    fn no_split_inside_numbers_links_or_abbreviation_free_text() {
        assert_eq!(
            units("It costs 3.50 today. See https://a.com/x.html now", Unit::Sentence),
            ["It costs 3.50 today.", "See https://a.com/x.html now"]
        );
    }

    #[test]
    fn paragraphs_split_at_blank_lines_only() {
        assert_eq!(units("one\nstill one\n\ntwo", Unit::Paragraph), ["one\nstill one", "two"]);
    }

    #[test]
    fn pieces_without_letters_are_kept_not_sent() {
        let segs = split("12345. ok", Unit::Sentence);
        assert_eq!(segs[0], Seg::Keep("12345."));
        assert_eq!(units("12345. ok", Unit::Sentence), ["ok"]);
    }

    #[test]
    fn joining_unchanged_units_gives_back_the_exact_text() {
        for text in ["", "  leading and trailing  ", "a. b!  c?\n\n\nd\n", "no end", "tab\tseparated.\tnext", "émoji 😀 done. ünï next."]
        {
            for unit in [Unit::Sentence, Unit::Paragraph] {
                assert_eq!(rebuilt(text, unit), text, "{text:?} as {unit:?}");
            }
        }
    }

    #[test]
    fn answers_replace_units_in_order() {
        let segs = split("a b.  c d.", Unit::Sentence);
        assert_eq!(join(&segs, &["A b.".into(), "C d.".into()]), "A b.  C d.");
    }
}
