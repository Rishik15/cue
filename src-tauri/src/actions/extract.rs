//! Purpose: Pull literal items out of text: email addresses, links and phone numbers.
//! Contents: emails / urls / phones — the matches in order of first appearance, one per line; url_spans — where links sit in a text (shared with `links`).

use std::collections::HashSet;
use std::ops::Range;
use std::sync::LazyLock;

use regex::Regex;

static EMAIL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)*\.[A-Za-z]{2,}").unwrap());
static URL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?i)\bhttps?://[^\s<>"']+"#).unwrap());
/// A run that starts like a number and may hold spaces, dots, dashes and brackets inside.
static PHONE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\+?\d[\d\s().-]{6,}\d").unwrap());
/// Day-month-year dates share the shape of a phone number; they are not one.
static DATE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d{1,4}[-./]\d{1,2}[-./]\d{1,4}$").unwrap());
/// Sentence punctuation that follows a link in prose but is almost never part of it.
const TRAILING: &[char] = &['.', ',', ';', ':', '!', '?', ')', ']', '}'];

pub fn emails(text: &str) -> String {
    unique(EMAIL.find_iter(text).map(|m| m.as_str().to_string()))
}

/// Phone numbers in `text`, one per line, without repeats. shortcut: any 7 to 15 digit run that is not a date counts, so a long reference number is taken for a phone; real validation needs a per-country format list.
pub fn phones(text: &str) -> String {
    let digits = |m: &str| m.chars().filter(char::is_ascii_digit).count();
    unique(PHONE.find_iter(text).map(|m| m.as_str().trim().to_string()).filter(|m| (7..=15).contains(&digits(m)) && !DATE.is_match(m)))
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
    let (mut seen, mut order) = (HashSet::new(), Vec::new());
    for item in items {
        if seen.insert(item.clone()) {
            order.push(item);
        }
    }
    order.join("\n")
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
    fn phone_numbers_are_found_but_dates_and_long_numbers_are_not() {
        let text = "Call +1 (555) 123-4567 or 555.987.6543, not 2026-10-14 nor 1234567890123456789.";
        assert_eq!(phones(text), "+1 (555) 123-4567\n555.987.6543");
    }

    #[test]
    fn links_lose_sentence_punctuation_but_keep_balanced_brackets() {
        let text = "See https://a.com/x?y=1. Also (https://b.com/p) and https://en.wikipedia.org/wiki/Rust_(language).";
        assert_eq!(urls(text), "https://a.com/x?y=1\nhttps://b.com/p\nhttps://en.wikipedia.org/wiki/Rust_(language)");
    }
}
