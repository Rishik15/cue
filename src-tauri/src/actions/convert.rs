//! Purpose: Conversions that need no model: identifier cases, plain quotes, line order and duplicates, and JSON layout.
//! Contents: camel_case / snake_case / kebab_case — per line; plain_quotes; sort_lines; dedupe_lines; json_pretty / json_minify — empty when the text is not JSON.

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

fn per_line(text: &str, f: impl Fn(&[String]) -> String) -> String {
    text.lines().map(|line| f(&words(line))).collect::<Vec<_>>().join("\n")
}

pub fn snake_case(text: &str) -> String {
    per_line(text, |w| w.join("_"))
}

pub fn kebab_case(text: &str) -> String {
    per_line(text, |w| w.join("-"))
}

fn capital(word: &str) -> String {
    word.chars().next().map(|c| c.to_uppercase().chain(word.chars().skip(1)).collect()).unwrap_or_default()
}

pub fn camel_case(text: &str) -> String {
    per_line(text, |w| w.first().cloned().into_iter().chain(w.iter().skip(1).map(|word| capital(word))).collect())
}

pub fn plain_quotes(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '‘' | '’' | '‚' | '‛' => '\'',
            '“' | '”' | '„' | '‟' => '"',
            other => other,
        })
        .collect()
}

pub fn sort_lines(text: &str) -> String {
    let mut lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    lines.sort_by_key(|l| l.to_lowercase());
    lines.join("\n")
}

/// Keeps the first of each repeated line (compared without surrounding blanks); blank lines stay where they are.
pub fn dedupe_lines(text: &str) -> String {
    let mut seen = HashSet::new();
    text.lines().filter(|l| l.trim().is_empty() || seen.insert(l.trim())).collect::<Vec<_>>().join("\n")
}

pub fn json_pretty(text: &str) -> String {
    serde_json::from_str::<serde_json::Value>(text.trim()).ok().and_then(|v| serde_json::to_string_pretty(&v).ok()).unwrap_or_default()
}

pub fn json_minify(text: &str) -> String {
    serde_json::from_str::<serde_json::Value>(text.trim()).ok().and_then(|v| serde_json::to_string(&v).ok()).unwrap_or_default()
}
