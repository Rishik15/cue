//! File: text/window.rs
//! Purpose: Cut a unit that is too big for one request into windows that fit, without losing or changing a character: the pieces and the gaps between them always add up to the original text.
//! Contents: refine — replace every unit that does not fit by windows; head — the first window of a text; prefix — the first characters of a text, so splitting never sees more than it can use; pack (private) — pack sentences, then clauses, then words, then characters into windows.
//! A window is a slice of the original starting and ending on a piece boundary (a sentence if it fits, else a clause, else a word, else single characters), so the joined result is exact.

use super::cancel::Cancel;
use super::chunk::{self, Seg};
use crate::actions::Unit;

/// The coarsest to the finest way to cut: sentences, clauses, words, characters.
const LEVELS: usize = 4;

/// The first `chars` characters of `text`, or all of it when it is shorter. Returns a slice that ends on a character boundary.
pub fn prefix(text: &str, chars: usize) -> &str {
    text.char_indices().nth(chars).map_or(text, |(end, _)| &text[..end])
}

/// Replaces every unit that does not fit by windows that do. Inputs: the segments, the quick test (a safe estimate), the exact test (a token count, tried only for a unit the quick test refuses) and the cancel token;
/// once it is set, the units not yet cut stay whole. Returns the segments; their texts still add up to the original.
pub fn refine<'a>(segs: Vec<Seg<'a>>, quick: &dyn Fn(&str) -> bool, exact: &dyn Fn(&str) -> bool, cancel: &Cancel) -> Vec<Seg<'a>> {
    segs.into_iter()
        .flat_map(|seg| match seg {
            Seg::Unit(unit) if !cancel.is_set() && !quick(unit) && !exact(unit) => pack(unit, 0, quick, cancel),
            other => vec![other],
        })
        .collect()
}

/// The first window of `text`: as much from its first piece with letters as fits, ending on the coarsest boundary that works. The caller passes only the part of the text a window can reach (`prefix`).
/// Returns the whole text when it fits, an empty slice for an empty text or a cancel.
pub fn head<'a>(text: &'a str, fits: &dyn Fn(&str) -> bool, cancel: &Cancel) -> &'a str {
    if fits(text) {
        return text;
    }
    pack(text, 0, fits, cancel).into_iter().find_map(|seg| if let Seg::Unit(window) = seg { Some(window) } else { None }).unwrap_or("")
}

/// The pieces of `text` at one cut level: units with the whitespace between them as keeps (characters have no gaps).
fn pieces(text: &str, level: usize) -> Vec<Seg<'_>> {
    match level {
        0 => chunk::split(text, Unit::Sentence),
        1 => cut(text, |c, next| matches!(c, ',' | ';' | ':') && next.is_none_or(char::is_whitespace)),
        2 => cut(text, |c, next| !c.is_whitespace() && next.is_none_or(char::is_whitespace)),
        _ => text.char_indices().map(|(i, c)| Seg::Unit(&text[i..i + c.len_utf8()])).collect(),
    }
}

/// Cuts after every character for which `ends(char, next char)` holds; the whitespace after a cut is a keep.
fn cut(text: &str, ends: impl Fn(char, Option<char>) -> bool) -> Vec<Seg<'_>> {
    let mut segs = Vec::new();
    let (mut start, mut chars) = (0, text.char_indices().peekable());
    while let Some((i, c)) = chars.next() {
        if !ends(c, chars.peek().map(|(_, n)| *n)) {
            continue;
        }
        let end = i + c.len_utf8();
        segs.push(Seg::Unit(&text[start..end]));
        let gap = text[end..].len() - text[end..].trim_start().len();
        if gap > 0 {
            segs.push(Seg::Keep(&text[end..end + gap]));
        }
        start = end + gap;
        while chars.peek().is_some_and(|(j, _)| *j < start) {
            chars.next();
        }
    }
    if start < text.len() {
        segs.push(Seg::Unit(&text[start..]));
    }
    segs
}

/// Packs the pieces of `text` into windows that fit. A piece that does not fit alone is cut at the next finer level. After a cancel the rest of the text is one keep. Returns segments covering `text` exactly.
fn pack<'a>(text: &'a str, level: usize, fits: &dyn Fn(&str) -> bool, cancel: &Cancel) -> Vec<Seg<'a>> {
    if fits(text) {
        return vec![Seg::Unit(text)];
    }
    let base = text.as_ptr() as usize;
    let (mut out, mut window, mut cursor) = (Vec::new(), None::<(usize, usize)>, 0);
    let flush = |out: &mut Vec<Seg<'a>>, window: &mut Option<(usize, usize)>, cursor: &mut usize| {
        if let Some((a, b)) = window.take() {
            if a > *cursor {
                out.push(Seg::Keep(&text[*cursor..a]));
            }
            out.push(Seg::Unit(&text[a..b]));
            *cursor = b;
        }
    };
    for piece in pieces(text, level) {
        if cancel.is_set() {
            break;
        }
        let Seg::Unit(p) = piece else { continue };
        let (s, e) = (p.as_ptr() as usize - base, p.as_ptr() as usize - base + p.len());
        if !fits(p) && level + 1 < LEVELS {
            flush(&mut out, &mut window, &mut cursor);
            if s > cursor {
                out.push(Seg::Keep(&text[cursor..s]));
            }
            out.extend(pack(p, level + 1, fits, cancel));
            cursor = e;
            continue;
        }
        if window.is_some_and(|(a, _)| fits(&text[a..e])) {
            window = window.map(|(a, _)| (a, e));
        } else {
            flush(&mut out, &mut window, &mut cursor);
            window = Some((s, e));
        }
    }
    flush(&mut out, &mut window, &mut cursor);
    if cursor < text.len() {
        out.push(Seg::Keep(&text[cursor..]));
    }
    out
}
