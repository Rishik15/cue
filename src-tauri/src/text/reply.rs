//! File: text/reply.rs
//! Purpose: Check what the model answered before it replaces a unit of the user's text, so a bad answer never overwrites good text: the unit is kept as it was instead.
//! Contents: Verdict and Reason (with a user-facing `explain`) — the outcome of a check; strip_wrapping — the production tidying of a raw answer; accept_for — tidy an answer and decide whether to trust it for a task (shared checks, then the task's own shape check from `verify`); max_tokens — the generation limit for an input; bounded_distance — banded character edit distance (tested).
//! An answer is rejected when it is empty, a refusal, runaway long, loses a link / email / number / mention / code span from the input, or (for corrections) changes more than the task allows,
//! which is what an answered question or an obeyed order looks like.

use std::collections::HashMap;

use regex::Regex;

use super::spans::{MASKABLE, PROTECTED};
use super::verify;
use crate::actions::{Keep, Task};
use crate::sanitize::strip_hidden;

/// Longest answer accepted, as a multiple of the input plus a margin for very short inputs. A longer answer is the model rambling, not editing.
const MAX_GROWTH: usize = 3;
/// Characters added to the allowed answer length, so a very short input may still grow.
const MARGIN: usize = 200;
/// A change this small is always fine, whatever the share: a 10-letter sentence with two fixes changes 40 % of it.
const FREE_EDITS: usize = 6;
/// Width of the first distance band; a typical correction is far inside it.
const FIRST_BAND: usize = 16;

/// Openings of a refusal, lower case: an answer that starts with one is refused unless the input does too.
const REFUSALS: [&str; 6] = ["i'm sorry", "i am sorry", "sorry,", "i can't", "i cannot", "as an ai"];
/// Phrases of an assistant talking to the user, not of an edited text: a small model that takes "Hello, I'm Sam." for a greeting answers "Hi Sam! How can I assist you today?" and one that takes "Thanks a lot!" for a thanks answers "You're welcome!". Refused anywhere in an answer unless the input has them too.
const ASSISTANT_TALK: [&str; 9] = [
    "how can i assist",
    "how can i help",
    "how may i",
    "i'm here to help",
    "i am here to help",
    "let me know if you",
    "is there anything else",
    "you're welcome",
    "you are welcome",
];

/// Removes what a model wraps around its answer: hidden characters, a code fence around plain text (its opening line may name a language), a lead-in line such as "Here is the corrected text:", and quotes the input did not have.
/// Inputs: the raw answer and the unit it answers. Returns the bare answer.
pub(crate) fn strip_wrapping(reply: &str, input: &str) -> String {
    let reply = strip_hidden(reply);
    let mut text = reply.trim();
    if let Some(inner) = text.strip_prefix("```") {
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

/// Why an answer was refused. `code` is what logs carry, never the user's text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    Empty,
    Refused,
    Runaway,
    LostProtected,
    OverEdited,
    NotBullets,
    NotShorter,
    LostNegation,
    MaskBroken,
}

impl Reason {
    /// A short stable code for logs.
    pub fn code(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Refused => "refused",
            Self::Runaway => "runaway",
            Self::LostProtected => "lost-protected",
            Self::OverEdited => "over-edited",
            Self::NotBullets => "not-bullets",
            Self::NotShorter => "not-shorter",
            Self::LostNegation => "lost-negation",
            Self::MaskBroken => "mask-broken",
        }
    }

    /// What to tell the user, as a short clause that completes "the answer ...".
    pub fn explain(self) -> &'static str {
        match self {
            Self::Empty => "was empty",
            Self::Refused => "was a refusal",
            Self::Runaway => "was far longer than the text",
            Self::LostProtected => "changed a link, number, path or mention",
            Self::OverEdited => "changed too much of the text",
            Self::NotBullets => "was not a list of dash lines",
            Self::NotShorter => "was not shorter",
            Self::LostNegation => "changed whether the text says not or will",
            Self::MaskBroken => "lost a protected part of the text",
        }
    }
}

/// The outcome of checking an answer: the tidied text to use, or the reason to keep the input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Accept(String),
    Reject(Reason),
}

impl Verdict {
    /// The accepted text, or None for a rejection.
    #[cfg(test)]
    pub fn text(&self) -> Option<&str> {
        match self {
            Self::Accept(text) => Some(text),
            Self::Reject(_) => None,
        }
    }
}

/// Character edit distance, case already folded by the caller, capped: returns the true distance when it is at most `limit`, otherwise `limit + 1`.
/// Equal inputs cost one comparison and a shared prefix and suffix are skipped. The band starts narrow and doubles up to `limit`, so the cost follows the real distance, not the limit.
pub fn bounded_distance(a: &[char], b: &[char], limit: usize) -> usize {
    let prefix = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    let (a, b) = (&a[prefix..], &b[prefix..]);
    let suffix = a.iter().rev().zip(b.iter().rev()).take_while(|(x, y)| x == y).count();
    let (a, b) = (&a[..a.len() - suffix], &b[..b.len() - suffix]);
    let mut band = FIRST_BAND.min(limit);
    loop {
        let found = banded(a, b, band);
        if found <= band {
            return found;
        }
        if band >= limit {
            return limit + 1;
        }
        band = (band * 2).min(limit);
    }
}

/// The edit distance of two trimmed slices when it is at most `limit`, otherwise `limit + 1`, computing only the `2 * limit + 1` cells around the diagonal of each row.
fn banded(a: &[char], b: &[char], limit: usize) -> usize {
    let cap = limit + 1;
    if a.is_empty() || b.is_empty() || a.len().abs_diff(b.len()) > limit {
        return (a.len().max(b.len())).min(cap);
    }
    let m = b.len();
    let mut prev: Vec<usize> = (0..=m).map(|j| j.min(cap)).collect();
    let mut cur = vec![cap; m + 1];
    for (i, ca) in (1usize..).zip(a) {
        let (lo, hi) = (i.saturating_sub(limit).max(1), (i + limit).min(m));
        cur[lo - 1] = if lo == 1 { i.min(cap) } else { cap };
        for j in lo..=hi {
            cur[j] = (prev[j - 1] + usize::from(*ca != b[j - 1])).min(prev[j] + 1).min(cur[j - 1] + 1).min(cap);
        }
        if hi < m {
            cur[hi + 1] = cap;
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[m]
}

/// A protected token without the punctuation, closing quotes and brackets that end a sentence or wrap a link around it.
fn trimmed<'h>(m: regex::Match<'h>) -> &'h str {
    m.as_str().trim_end_matches(['.', ',', ';', ':', '!', '?', ')', ']', '}', '>', '"', '\'', '\u{201d}', '\u{2019}', '\u{bb}'])
}

/// Whether every token of `input` that `spans` finds appears in `answer` at least as many times: a whole-token multiset comparison in one linear scan of each.
fn keeps_protected(spans: &Regex, input: &str, answer: &str) -> bool {
    let mut available: HashMap<&str, usize> = HashMap::new();
    for token in spans.find_iter(answer) {
        *available.entry(trimmed(token)).or_default() += 1;
    }
    spans.find_iter(input).all(|token| match available.get_mut(trimmed(token)) {
        Some(left) if *left > 0 => {
            *left -= 1;
            true
        }
        _ => false,
    })
}

/// Whether the answer keeps the tokens the task requires. Inputs: the task's `Keep`, the input and the answer. Returns true when nothing required was lost.
fn survives(keep: Keep, input: &str, answer: &str) -> bool {
    match keep {
        Keep::All => keeps_protected(&PROTECTED, input, answer),
        Keep::Spans => keeps_protected(&MASKABLE, input, answer),
        Keep::Nothing => true,
    }
}

/// Whether a corrected answer changed more of the input than `share` allows. Inputs: input, answer, the allowed share. Returns true when it changed too much.
fn over_edited(input: &str, answer: &str, share: f32) -> bool {
    let (a, b): (Vec<char>, Vec<char>) = (input.to_lowercase().chars().collect(), answer.to_lowercase().chars().collect());
    let allowed = ((a.len().max(b.len()) as f32 * share).ceil() as usize).max(FREE_EDITS);
    bounded_distance(&a, &b, allowed) > allowed
}

/// Tidies `answer` and applies the shared checks. Inputs: the unit sent, the model's raw answer, the share of the text a correction may change (None for no limit), and which tokens must survive (`Keep`).
/// Returns Accept(text) or Reject(reason).
fn judge(input: &str, answer: &str, max_change: Option<f32>, keep: Keep) -> Verdict {
    let text = strip_wrapping(answer, input);
    let lower = text.to_lowercase();
    let said = input.to_lowercase();
    let refused = REFUSALS.iter().any(|r| lower.starts_with(r) && !said.starts_with(r))
        || ASSISTANT_TALK.iter().any(|p| lower.contains(p) && !said.contains(p));
    let reason = if text.is_empty() {
        Reason::Empty
    } else if refused {
        Reason::Refused
    } else if text.chars().count() > input.chars().count() * MAX_GROWTH + MARGIN {
        Reason::Runaway
    } else if !survives(keep, input, &text) {
        Reason::LostProtected
    } else if max_change.is_some_and(|share| over_edited(input, &text, share)) {
        Reason::OverEdited
    } else {
        return Verdict::Accept(text);
    };
    Verdict::Reject(reason)
}

/// The shared checks with every protected token required, for tests of the checks alone.
#[cfg(test)]
pub fn accept(input: &str, answer: &str, max_change: Option<f32>) -> Verdict {
    judge(input, answer, max_change, Keep::All)
}

/// Judges an answer for a task: the shared checks as the task asks for them, then the task's own shape check. Inputs: the task, the unit sent and the raw answer. Returns Accept(text) or Reject(reason).
pub fn accept_for(task: &Task, input: &str, answer: &str) -> Verdict {
    match judge(input, answer, task.max_change, task.keep) {
        Verdict::Accept(text) => verify::check(task.check, input, &text).map_or(Verdict::Accept(text), Verdict::Reject),
        rejected => rejected,
    }
}

/// About 4 characters per token for English; twice the input leaves room for a longer rewrite without letting the model run on.
pub fn max_tokens(input: &str) -> usize {
    (input.chars().count() / 2 + 80).min(2048)
}
