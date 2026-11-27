//! File: actions/extract.rs
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

/// Email addresses in `text`, one per line, without repeats; the text itself when there are none, so nothing found changes nothing.
pub fn emails(text: &str) -> String {
    unique(text, EMAIL.find_iter(text).map(|m| m.as_str().to_string()))
}

/// Phone numbers in `text`, one per line, without repeats; the text itself when there are none. shortcut: any 7 to 15 digit run that is not a date counts, so a long reference number is taken for a phone; real validation needs a per-country format list.
pub fn phones(text: &str) -> String {
    let digits = |m: &str| m.chars().filter(char::is_ascii_digit).count();
    unique(
        text,
        PHONE.find_iter(text).map(|m| m.as_str().trim().to_string()).filter(|m| (7..=15).contains(&digits(m)) && !DATE.is_match(m)),
    )
}

/// Links in `text`, one per line, without repeats; the text itself when there are none.
pub fn urls(text: &str) -> String {
    unique(text, url_spans(text).into_iter().map(|r| text[r].to_string()))
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

/// The opening bracket of a closing one. Input: the closing character. Return: its opener.
fn open_of(close: char) -> char {
    match close {
        ')' => '(',
        ']' => '[',
        _ => '{',
    }
}

/// The items without repeats, one per line. Inputs: the text they came from and the items. Return: the list, or `text` when there were no items (an empty list would erase the text).
fn unique(text: &str, items: impl Iterator<Item = String>) -> String {
    let (mut seen, mut order) = (HashSet::new(), Vec::new());
    for item in items {
        if seen.insert(item.clone()) {
            order.push(item);
        }
    }
    if order.is_empty() {
        return text.to_string();
    }
    order.join("\n")
}
