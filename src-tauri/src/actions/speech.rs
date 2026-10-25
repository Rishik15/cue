//! Purpose: Clean-up of spoken text that needs no model: filler words, a word said twice, and numbers said in words.
//! Contents: remove_fillers / remove_repeats — drop words from each line; spoken_numbers — "twenty five" to 25 (up to 999 999; a lone one to nine stays a word); Kind / number_word / parse_group — the number grammar.

/// Said while thinking. Not "mm": that is millimetres.
const FILLERS: &[&str] = &["um", "umm", "uh", "uhh", "uhm", "er", "erm", "hm", "hmm"];

pub fn remove_fillers(text: &str) -> String {
    drop_words(text, |word, _| FILLERS.contains(&word))
}

/// `text` without each alphabetic word that repeats the previous kept word on the same line (compared case-insensitively, ignoring surrounding punctuation); returns the cleaned text. shortcut: a deliberate repeat ("that that", "had had") is removed too; telling it apart needs grammar.
pub fn remove_repeats(text: &str) -> String {
    drop_words(text, |word, previous| word == previous && word.chars().all(char::is_alphabetic))
}

/// Drops every word `unwanted(word, previous kept word)` says to (both lowercase, without punctuation) from each line. A dropped word's sentence end moves to the word before it,
/// and a capital it carried passes to the next word, so "Um, so it works." keeps its shape.
fn drop_words(text: &str, mut unwanted: impl FnMut(&str, &str) -> bool) -> String {
    text.lines().map(|line| drop_in_line(line, &mut unwanted)).collect::<Vec<_>>().join("\n")
}

fn drop_in_line(line: &str, unwanted: &mut impl FnMut(&str, &str) -> bool) -> String {
    let (mut kept, mut previous, mut capitalize): (Vec<String>, String, bool) = (Vec::new(), String::new(), false);
    for token in line.split_whitespace() {
        let word = token.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
        if !word.is_empty() && unwanted(&word, &previous) {
            capitalize |= token.starts_with(char::is_uppercase) && kept.last().is_none_or(|l| l.ends_with(['.', '!', '?']));
            if let (Some(end), Some(last)) = (token.chars().rev().find(|c| matches!(c, '.' | '!' | '?')), kept.last_mut()) {
                if !last.ends_with(['.', '!', '?']) {
                    last.push(end);
                }
            }
            continue;
        }
        let mut token = token.to_string();
        if std::mem::take(&mut capitalize) {
            token = capitalized(&token);
        }
        previous = word;
        kept.push(token);
    }
    kept.join(" ")
}

fn capitalized(token: &str) -> String {
    let mut chars = token.chars();
    chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Unit,
    Teen,
    Tens,
    Hundred,
    Thousand,
}

fn number_word(word: &str) -> Option<(u32, Kind)> {
    const UNITS: [&str; 9] = ["one", "two", "three", "four", "five", "six", "seven", "eight", "nine"];
    const TEENS: [&str; 10] =
        ["ten", "eleven", "twelve", "thirteen", "fourteen", "fifteen", "sixteen", "seventeen", "eighteen", "nineteen"];
    const TENS: [&str; 8] = ["twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety"];
    let at = |list: &[&str]| list.iter().position(|w| *w == word).map(|i| i as u32);
    if let Some(i) = at(&UNITS) {
        return Some((i + 1, Kind::Unit));
    }
    if let Some(i) = at(&TEENS) {
        return Some((10 + i, Kind::Teen));
    }
    if let Some(i) = at(&TENS) {
        return Some((20 + 10 * i, Kind::Tens));
    }
    match word {
        "hundred" => Some((100, Kind::Hundred)),
        "thousand" => Some((1000, Kind::Thousand)),
        _ => None,
    }
}

/// The value of a run of number words, or None when they do not form one number ("one two three" is three numbers, not six).
fn parse_group(words: &[(u32, Kind)]) -> Option<u32> {
    let (mut total, mut current, mut last): (u32, u32, Option<Kind>) = (0, 0, None);
    for &(value, kind) in words {
        let fits = match kind {
            Kind::Unit => matches!(last, None | Some(Kind::Tens | Kind::Hundred | Kind::Thousand)),
            Kind::Teen | Kind::Tens => matches!(last, None | Some(Kind::Hundred | Kind::Thousand)),
            Kind::Hundred => matches!(last, Some(Kind::Unit | Kind::Teen)),
            Kind::Thousand => matches!(last, Some(Kind::Unit | Kind::Teen | Kind::Tens | Kind::Hundred)),
        };
        if !fits {
            return None;
        }
        match kind {
            Kind::Hundred => current = current.checked_mul(100)?,
            Kind::Thousand => {
                total = total.checked_add(current.checked_mul(1000)?)?;
                current = 0;
            }
            _ => current = current.checked_add(value)?,
        }
        last = Some(kind);
    }
    total.checked_add(current)
}

/// A word split into the punctuation before it, the word and the punctuation after it.
fn split_token(token: &str) -> (&str, &str, &str) {
    let start = token.find(char::is_alphanumeric).unwrap_or(token.len());
    let end = token.rfind(char::is_alphanumeric).map_or(start, |i| i + token[i..].chars().next().map_or(1, char::len_utf8));
    (&token[..start], &token[start..end], &token[end..])
}

pub fn spoken_numbers(text: &str) -> String {
    text.lines().map(numbers_in_line).collect::<Vec<_>>().join("\n")
}

fn numbers_in_line(line: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut run: Vec<(&str, (u32, Kind))> = Vec::new(); // the tokens of the number words seen so far
    for token in line.split_whitespace() {
        let (_, word, tail) = split_token(token);
        match number_word(&word.to_lowercase()) {
            Some(n) => {
                run.push((token, n));
                if !tail.is_empty() {
                    flush(&mut run, &mut out);
                }
            }
            None => {
                flush(&mut run, &mut out);
                out.push(token.to_string());
            }
        }
    }
    flush(&mut run, &mut out);
    out.join(" ")
}

/// Writes the pending run as digits when it is one number worth writing as digits, otherwise as it was said.
fn flush(run: &mut Vec<(&str, (u32, Kind))>, out: &mut Vec<String>) {
    let words: Vec<(u32, Kind)> = run.iter().map(|(_, n)| *n).collect();
    let worth = run.len() > 1 || words.first().is_some_and(|(_, kind)| *kind != Kind::Unit);
    match parse_group(&words).filter(|_| worth) {
        Some(value) => {
            let lead = run.first().map_or("", |(t, _)| split_token(t).0);
            let tail = run.last().map_or("", |(t, _)| split_token(t).2);
            out.push(format!("{lead}{value}{tail}"));
        }
        None => out.extend(run.iter().map(|(t, _)| t.to_string())),
    }
    run.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fillers_go_and_the_sentence_keeps_its_shape() {
        assert_eq!(remove_fillers("Um, so I uh think it works. Hmm."), "So I think it works.");
        assert_eq!(remove_fillers("it is 5 mm wide"), "it is 5 mm wide");
    }

    #[test]
    fn a_word_said_twice_is_said_once() {
        assert_eq!(remove_repeats("I I think the the plan, plan works"), "I think the plan, works");
        assert_eq!(remove_repeats("1 1 and 2"), "1 1 and 2");
    }

    #[test]
    fn spoken_numbers_become_digits_except_a_lone_small_one() {
        assert_eq!(spoken_numbers("I have twenty five apples."), "I have 25 apples.");
        assert_eq!(spoken_numbers("in two thousand twenty six, one hundred and five"), "in 2026, 100 and five");
        assert_eq!(spoken_numbers("one of the three"), "one of the three");
        assert_eq!(spoken_numbers("one two three"), "one two three");
    }
}
