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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifier_cases_split_words_and_existing_humps() {
        assert_eq!(snake_case("Hello World\nuserName id"), "hello_world\nuser_name_id");
        assert_eq!(kebab_case("Make it so!"), "make-it-so");
        assert_eq!(camel_case("hello big_world-now"), "helloBigWorldNow");
    }

    #[test]
    fn quotes_become_plain() {
        assert_eq!(plain_quotes("“a” ‘b’ it’s"), "\"a\" 'b' it's");
    }

    #[test]
    fn lines_sort_without_regard_to_case_and_lose_repeats() {
        assert_eq!(sort_lines("pear\nApple\n\nbanana"), "Apple\nbanana\npear");
        assert_eq!(dedupe_lines("a\nb\n a\n\nb\nc"), "a\nb\n\nc");
    }

    #[test]
    fn json_changes_only_when_valid_and_keeps_key_order() {
        assert_eq!(json_minify("{ \"b\": 1,\n \"a\": [1, 2] }"), "{\"b\":1,\"a\":[1,2]}");
        assert_eq!(json_pretty("{\"b\":1}"), "{\n  \"b\": 1\n}");
        assert_eq!(json_pretty("not json"), "");
    }
}
