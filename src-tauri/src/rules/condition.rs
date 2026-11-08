//! File: condition.rs
//! Purpose: Turn a stored `Condition` into something cheap to test, and test it.
//! Contents: Compiled — a condition ready to evaluate (regexes built once); compile — validate and build; test — evaluate on a text; errors name the problem, never the pattern, so the log holds no user text;
//! cost — the order cheap checks run in; needs_lower — whether a text must be lower-cased first; looks_like — URL / email / JSON / list detection.

use regex::{Regex, RegexBuilder};

use super::model::{Condition, Kind};

pub enum Compiled {
    Length { over: bool, chars: usize },
    Contains(String),
    StartsWith(String),
    EndsWith(String),
    LooksLike(Kind),
    Matches(Regex),
}

/// Builds a stored condition into its checkable form. Input: the condition. Returns it, or Err with a message that never quotes the user's pattern.
pub fn compile(c: &Condition) -> Result<Compiled, String> {
    Ok(match c {
        Condition::Contains { value } => Compiled::Contains(value.to_lowercase()),
        Condition::StartsWith { value } => Compiled::StartsWith(value.to_lowercase()),
        Condition::EndsWith { value } => Compiled::EndsWith(value.to_lowercase()),
        Condition::LongerThan { chars } => Compiled::Length { over: true, chars: *chars },
        Condition::ShorterThan { chars } => Compiled::Length { over: false, chars: *chars },
        Condition::LooksLike { kind } => Compiled::LooksLike(*kind),
        // size_limit keeps a pasted pathological pattern from taking memory; the regex crate itself cannot backtrack catastrophically.
        Condition::Matches { pattern } => Compiled::Matches(
            RegexBuilder::new(pattern)
                .case_insensitive(true)
                .size_limit(64 << 10)
                .build()
                .map_err(|_| "A pattern is not valid or too complex. Check its brackets and escapes.".to_string())?,
        ),
    })
}

/// Cheapest first: length, plain text, shape checks, then regex.
pub fn cost(c: &Compiled) -> u8 {
    match c {
        Compiled::Length { .. } => 0,
        Compiled::Contains(_) | Compiled::StartsWith(_) | Compiled::EndsWith(_) => 1,
        Compiled::LooksLike(_) => 2,
        Compiled::Matches(_) => 3,
    }
}

/// Whether the plain-text conditions need the lower-cased text, which the caller makes once per rule instead of once per condition.
pub fn needs_lower(c: &Compiled) -> bool {
    matches!(c, Compiled::Contains(_) | Compiled::StartsWith(_) | Compiled::EndsWith(_))
}

/// `lower` is `text.to_lowercase()`; it is only read by the plain-text conditions.
pub fn test(c: &Compiled, text: &str, lower: &str) -> bool {
    match c {
        Compiled::Length { over, chars } => (text.chars().count() > *chars) == *over,
        Compiled::Contains(v) => lower.contains(v.as_str()),
        Compiled::StartsWith(v) => lower.trim_start().starts_with(v.as_str()),
        Compiled::EndsWith(v) => lower.trim_end().ends_with(v.as_str()),
        Compiled::LooksLike(kind) => looks_like(*kind, text.trim()),
        Compiled::Matches(re) => re.is_match(text),
    }
}

/// Whether a text has a shape (URL, email, JSON, list). Inputs: the shape and the text. Returns the verdict.
fn looks_like(kind: Kind, text: &str) -> bool {
    match kind {
        Kind::Url => (text.starts_with("http://") || text.starts_with("https://")) && !text.contains(char::is_whitespace),
        Kind::Email => text.split_once('@').is_some_and(|(user, host)| {
            !user.is_empty() && !text.contains(char::is_whitespace) && host.contains('.') && !host.ends_with('.') && !host.contains('@')
        }),
        Kind::Json => text.starts_with(['{', '[']) && serde_json::from_str::<serde_json::Value>(text).is_ok(),
        Kind::List => {
            let lines: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
            lines.len() >= 2 && lines.iter().all(|l| is_item(l))
        }
    }
}

/// Whether a line is a list item (bullet or number). Input: the line. Returns the verdict.
fn is_item(line: &str) -> bool {
    let rest = line.trim_start_matches(|c: char| c.is_ascii_digit());
    line.starts_with(['-', '*', '•']) || (rest.len() < line.len() && rest.starts_with(['.', ')']))
}
