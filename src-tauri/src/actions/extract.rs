//! Purpose: Pull literal items out of text: email addresses and links.
//! Contents: emails / urls — the matches in order of first appearance, one per line; url_spans — where links sit in a text (shared with `links`).

use std::ops::Range;
use std::sync::LazyLock;

use regex::Regex;

static EMAIL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)*\.[A-Za-z]{2,}").unwrap());
static URL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?i)\bhttps?://[^\s<>"']+"#).unwrap());
/// Sentence punctuation that follows a link in prose but is almost never part of it.
const TRAILING: &[char] = &['.', ',', ';', ':', '!', '?', ')', ']', '}'];

pub fn emails(text: &str) -> String {
    unique(EMAIL.find_iter(text).map(|m| m.as_str().to_string()))
}

pub fn urls(text: &str) -> String {
    unique(url_spans(text).into_iter().map(|r| text[r].to_string()))
}

/// Byte ranges of the links in `text`, without the punctuation that ends a sentence. A closing bracket stays when the link opened one
/// (Wikipedia-style `Rust_(language)`).
pub fn url_spans(text: &str) -> Vec<Range<usize>> {
    URL.find_iter(text)
        .map(|m| {
            let mut end = m.end();
            while let Some(c) = text[m.start()..end].chars().next_back().filter(|c| TRAILING.contains(c)) {
                let link = &text[m.start()..end];
                let balanced = matches!(c, ')' | ']' | '}') && link.matches(open_of(c)).count() >= link.matches(c).count();
                if balanced {
                    break;
                }
                end -= c.len_utf8();
            }
            m.start()..end
        })
        .collect()
}

fn open_of(close: char) -> char {
    match close {
        ')' => '(',
        ']' => '[',
        _ => '{',
    }
}

fn unique(items: impl Iterator<Item = String>) -> String {
    let mut seen = Vec::new();
    for item in items {
        if !seen.contains(&item) {
            seen.push(item);
        }
    }
    seen.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_emails_once_in_order() {
        let text = "Write a.b+c@mail.example.com or x@y.io, then x@y.io again.";
        assert_eq!(emails(text), "a.b+c@mail.example.com\nx@y.io");
    }

    #[test]
    fn links_lose_sentence_punctuation_but_keep_balanced_brackets() {
        let text = "See https://a.com/x?y=1. Also (https://b.com/p) and https://en.wikipedia.org/wiki/Rust_(language).";
        assert_eq!(urls(text), "https://a.com/x?y=1\nhttps://b.com/p\nhttps://en.wikipedia.org/wiki/Rust_(language)");
    }
}
