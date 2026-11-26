//! File: convert.rs
//! Purpose: Conversions that need no model: identifier cases, plain quotes, line order and duplicates, and JSON layout.
//! Contents: camel_case / snake_case / kebab_case — per line; plain_quotes; sort_lines; dedupe_lines; json_pretty / json_minify — the text unchanged when it is not JSON.

use super::format::line_breaks;
use std::collections::HashSet;

/// The words of a line, lowercase: split at anything that is not a letter or digit and where a lowercase letter meets a capital (`userName`).
fn words(line: &str) -> Vec<String> {
    let (mut out, mut word, mut prev_lower) = (Vec::new(), String::new(), false);
    for c in line.chars() {
        if (!c.is_alphanumeric() || (c.is_uppercase() && prev_lower)) && !word.is_empty() {
            out.push(std::mem::take(&mut word));
        }
        if c.is_alphanumeric() {
            word.extend(c.to_lowercase());
        }
        prev_lower = c.is_lowercase();
    }
    if !word.is_empty() {
        out.push(word);
    }
    out
}

/// Applies `f` to the words of each line. Inputs: the text and the function that builds one line from its words. Return: the lines joined by newlines.
fn per_line(text: &str, f: impl Fn(&[String]) -> String) -> String {
    text.lines().map(|line| f(&words(line))).collect::<Vec<_>>().join("\n")
}

/// The text as snake_case, per line. Input: the text. Return: the converted text.
pub fn snake_case(text: &str) -> String {
    per_line(text, |w| w.join("_"))
}

/// The text as kebab-case, per line. Input: the text. Return: the converted text.
pub fn kebab_case(text: &str) -> String {
    per_line(text, |w| w.join("-"))
}

/// The word with its first letter upper case. Input: the word. Return: the new word.
fn capital(word: &str) -> String {
    word.chars().next().map(|c| c.to_uppercase().chain(word.chars().skip(1)).collect()).unwrap_or_default()
}

/// The text as camelCase, per line. Input: the text. Return: the converted text.
pub fn camel_case(text: &str) -> String {
    per_line(text, |w| w.first().cloned().into_iter().chain(w.iter().skip(1).map(|word| capital(word))).collect())
}

/// Straight quotes for curly ones. Input: the text. Return: the text with ' and " only.
pub fn plain_quotes(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '‘' | '’' | '‚' | '‛' => '\'',
            '“' | '”' | '„' | '‟' => '"',
            other => other,
        })
        .collect()
}

/// The non-blank lines in case-insensitive order; lines end as in `line_breaks`, so a bare CR is a line break. Input: the text. Return: the sorted lines.
pub fn sort_lines(text: &str) -> String {
    let mut lines: Vec<&str> = line_breaks(text).filter(|l| !l.trim().is_empty()).collect();
    lines.sort_by_key(|l| l.to_lowercase());
    lines.join("\n")
}

/// Keeps the first of each repeated line, compared without surrounding blanks. Each line moves with its own ending, so CRLF pairs stay whole and blank lines stay where they are; a second run changes nothing. Input: the text. Return: the text without repeats.
pub fn dedupe_lines(text: &str) -> String {
    let mut seen = HashSet::new();
    text.split_inclusive('\n').filter(|l| l.trim().is_empty() || seen.insert(l.trim())).collect()
}

/// Indents valid JSON. Input: the text. Return: the indented JSON, or the text itself when it is not JSON.
pub fn json_pretty(text: &str) -> String {
    serde_json::from_str::<serde_json::Value>(text.trim())
        .ok()
        .and_then(|v| serde_json::to_string_pretty(&v).ok())
        .unwrap_or_else(|| text.to_string())
}

/// Puts valid JSON on one line. Input: the text. Return: the one-line JSON, or the text itself when it is not JSON.
pub fn json_minify(text: &str) -> String {
    serde_json::from_str::<serde_json::Value>(text.trim())
        .ok()
        .and_then(|v| serde_json::to_string(&v).ok())
        .unwrap_or_else(|| text.to_string())
}
