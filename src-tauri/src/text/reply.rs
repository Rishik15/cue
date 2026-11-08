//! Purpose: Check what the model answered before it replaces a unit of the user's text, so a bad answer never overwrites good text: the unit is kept as it was instead.
//! Contents: accept — tidy an answer and decide whether to trust it; max_tokens — the generation limit for an input; changed_chars — character edit distance (tested).
//! An answer is rejected when it is empty, a refusal, runaway long, loses a link / email / number / mention / code span from the input, or (for corrections) changes more than the task allows,
//! which is what an answered question or an obeyed order looks like.

use std::sync::LazyLock;

use regex::Regex;

use crate::sanitize::strip_hidden;

/// Longest answer accepted, as a multiple of the input plus a margin for very short inputs. A longer answer is the model rambling, not editing.
const MAX_GROWTH: usize = 3;
const MARGIN: usize = 200;
/// A change this small is always fine, whatever the share: a 10-letter sentence with two fixes changes 40 % of it.
const FREE_EDITS: usize = 6;

const REFUSALS: [&str; 6] = ["i'm sorry", "i am sorry", "sorry,", "i can't", "i cannot", "as an ai"];

/// What must survive an edit untouched: links, emails, @mentions and #tags, numbers (with their separators) and `code`.
static PROTECTED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"https?://\S+|[\w.+-]+@[\w-]+(?:\.[\w-]+)+|[@#]\w+|\d(?:[\d.,:/-]*\d)?|`[^`\n]+`").expect("valid pattern")
});

fn strip_wrapping(reply: &str, input: &str) -> String {
    let reply = strip_hidden(reply); // nothing hidden comes back from the model either
    let mut text = reply.trim();
    if let Some(inner) = text.strip_prefix("```") {
        // A code fence around plain text: drop the opening line (it may name a language) and the closing fence.
        text = inner.split_once('\n').map_or(inner, |(_, body)| body).trim_end().trim_end_matches("```").trim();
    }
    let first_line_end = text.find('\n').unwrap_or(text.len());
    let first = text[..first_line_end].to_lowercase();
    if first.ends_with(':') && ["here is", "here's", "sure", "certainly", "of course"].iter().any(|p| first.starts_with(p)) {
        text = text[first_line_end..].trim();
    }
    let quoted =
        |s: &str| s.len() > 1 && ((s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\u{201c}') && s.ends_with('\u{201d}')));
    if quoted(text) && !quoted(input.trim()) {
        text = text.trim_matches(|c| c == '"' || c == '\u{201c}' || c == '\u{201d}').trim();
    }
    text.to_string()
}

/// Character edit distance, case ignored, with two rows of memory.
pub fn changed_chars(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.to_lowercase().chars().collect(), b.to_lowercase().chars().collect());
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            cur[j + 1] = (prev[j] + usize::from(ca != cb)).min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

fn keeps_protected(input: &str, answer: &str) -> bool {
    PROTECTED.find_iter(input).all(|m| answer.contains(m.as_str().trim_end_matches(['.', ',', ';', ':', '!', '?', ')'])))
}

pub fn accept(input: &str, answer: &str, max_change: Option<f32>) -> Option<String> {
    let text = strip_wrapping(answer, input);
    let lower = text.to_lowercase();
    let refused = REFUSALS.iter().any(|r| lower.starts_with(r) && !input.to_lowercase().starts_with(r));
    let runaway = text.chars().count() > input.chars().count() * MAX_GROWTH + MARGIN;
    if text.is_empty() || refused || runaway || !keeps_protected(input, &text) {
        return None;
    }
    if let Some(share) = max_change {
        let (n_in, n_out) = (input.chars().count(), text.chars().count());
        let allowed = ((n_in.max(n_out) as f32 * share).ceil() as usize).max(FREE_EDITS);
        // The distance is at least the difference in length, so a big difference is rejected without the quadratic work.
        if n_in.abs_diff(n_out) > allowed || changed_chars(input, &text) > allowed {
            return None;
        }
    }
    Some(text)
}

/// About 4 characters per token for English; twice the input leaves room for a longer rewrite without letting the model run on.
pub fn max_tokens(input: &str) -> usize {
    (input.chars().count() / 2 + 80).min(2048)
}
