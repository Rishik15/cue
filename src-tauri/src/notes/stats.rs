//! Purpose: Counts for a note or a selection: how many numbers, their sum and average, lines, words, characters and how easy the text is to read. The status line shows them for `sum`, `avg` and `count` notes and for any selection.
//! Contents: of — text to Stats; Stats — the counts, the sum and average already written as the note writes numbers; keyword — the first-line word that asks for them.

use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;

use super::math::{show, Look};

#[derive(Serialize)]
pub struct Stats {
    numbers: usize,
    sum: Option<String>,
    avg: Option<String>,
    lines: usize,
    words: usize,
    chars: usize,
    /// Flesch reading ease, 0 to 100 (higher is easier); None for a few words.
    ease: Option<i32>,
}

/// A number not stuck to a letter ("O2" is a name, "5km" is 5): optional minus, thousands commas, decimals.
static NUMBER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?:^|[^\w.])(-?\d+(?:,\d{3})*(?:\.\d+)?)").expect("valid pattern"));

/// `sum`, `avg` or `count` on the first line.
pub fn keyword(first_line: &str) -> bool {
    matches!(first_line.trim().to_lowercase().as_str(), "sum" | "avg" | "count")
}

/// Counts the text; `//` lines are left out.
pub fn of(text: &str, look: &Look) -> Stats {
    let kept: Vec<&str> = text.lines().filter(|l| !l.trim_start().starts_with("//")).collect();
    let numbers: Vec<f64> = kept.iter().flat_map(|l| NUMBER.captures_iter(l)).filter_map(|c| c[1].replace(',', "").parse().ok()).collect();
    let sum: f64 = numbers.iter().sum();
    let words: Vec<&str> = kept.iter().flat_map(|l| l.split_whitespace()).collect();
    Stats {
        numbers: numbers.len(),
        sum: (!numbers.is_empty()).then(|| show(sum, look)),
        avg: (!numbers.is_empty()).then(|| show(sum / numbers.len() as f64, look)),
        lines: kept.iter().filter(|l| !l.trim().is_empty()).count(),
        words: words.len(),
        chars: kept.iter().map(|l| l.chars().count()).sum::<usize>() + kept.len().saturating_sub(1),
        ease: reading_ease(&kept.join("\n"), &words),
    }
}

fn reading_ease(text: &str, words: &[&str]) -> Option<i32> {
    if words.len() < 5 {
        return None;
    }
    let sentences = text.split(['.', '!', '?']).filter(|s| s.chars().any(char::is_alphanumeric)).count().max(1) as f64;
    let syllables: usize = words.iter().map(|w| syllables(w)).sum();
    let count = words.len() as f64;
    let ease = 206.835 - 1.015 * (count / sentences) - 84.6 * (syllables as f64 / count);
    Some(ease.clamp(0.0, 100.0).round() as i32)
}

/// Groups of vowels, less a silent final e; at least one.
fn syllables(word: &str) -> usize {
    let word = word.to_lowercase();
    let letters: Vec<char> = word.chars().filter(|c| c.is_alphabetic()).collect();
    let vowel = |c: &char| "aeiouy".contains(*c);
    let groups = letters.iter().enumerate().filter(|(i, c)| vowel(c) && (*i == 0 || !vowel(&letters[i - 1]))).count();
    let silent = letters.len() > 2 && letters.last() == Some(&'e') && !letters.get(letters.len() - 2).is_some_and(vowel);
    (groups - usize::from(silent && groups > 1)).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOOK: Look = Look { digits: 1, thousands: true };

    #[test]
    fn sums_and_averages_numbers_not_names() {
        let s = of("apples 1,200\nO2 level 3\n// 1000\n-2.5 and 5km", &LOOK);
        assert_eq!((s.numbers, s.sum.as_deref(), s.avg.as_deref()), (4, Some("1,205.5"), Some("301.4")));
        assert_eq!(s.lines, 3);
    }

    #[test]
    fn counts_words_and_chars_and_has_no_numbers_for_prose() {
        let s = of("one two\nthree", &LOOK);
        assert_eq!((s.words, s.chars, s.lines, s.numbers), (3, 13, 2, 0));
        assert!(s.sum.is_none());
        assert!(s.ease.is_none());
    }

    #[test]
    fn plain_text_is_easy_and_dense_text_is_hard() {
        let easy = of("The cat sat on the mat. The dog ran to the park.", &LOOK).ease.unwrap();
        let hard = of(
            "Notwithstanding unprecedented organizational complexity, interdepartmental communication deteriorated considerably.",
            &LOOK,
        )
        .ease
        .unwrap();
        assert!(easy > 80 && hard < 20, "{easy} {hard}");
        assert!(keyword(" Sum ") && !keyword("summary"));
    }
}
