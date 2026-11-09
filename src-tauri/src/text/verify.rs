//! File: text/verify.rs
//! Purpose: The answer check that belongs to one kind of task, beyond the shared token and change-share checks: bullets must be dash lines, a shorter text must not be longer, a correction must not flip a negation or a future.
//! Contents: check — run the task's check on an input and its answer; has_word — whether a word list occurs in a text.
//! shortcut: tense is checked only for the future marker "will" and negation only as present-or-absent (so "no" to "any" in a double negative stays legal); a full tense check needs a parser.

use super::reply::Reason;
use crate::actions::Check;

/// Negation words, apostrophes removed ("don't" is "dont"), so a fixed contraction counts the same as the broken one.
const NEGATIONS: [&str; 24] = [
    "not", "no", "never", "nobody", "nothing", "none", "without", "dont", "doesnt", "didnt", "cant", "cannot", "wont", "isnt", "arent", "wasnt",
    "werent", "hasnt", "havent", "hadnt", "shouldnt", "wouldnt", "couldnt", "aint",
];
const FUTURE: [&str; 2] = ["will", "gonna"];

/// Whether any word of `list` occurs in `text` (lower case, apostrophes removed, split on non-letters).
pub fn has_word(text: &str, list: &[&str]) -> bool {
    let plain: String = text.to_lowercase().chars().filter(|c| !matches!(c, '\'' | '\u{2019}')).collect();
    plain.split(|c: char| !c.is_alphabetic()).any(|word| list.contains(&word))
}

/// Whether the text has a future marker: will, gonna or a contracted 'll.
fn future(text: &str) -> bool {
    has_word(text, &FUTURE) || text.contains("'ll") || text.contains("\u{2019}ll")
}

/// Whether every non-empty line starts with a dash and a space, and there is at least one line.
fn dash_lines(answer: &str) -> bool {
    let mut lines = answer.lines().map(str::trim).filter(|l| !l.is_empty()).peekable();
    lines.peek().is_some() && lines.all(|l| l.starts_with("- "))
}

/// Runs the check of a task. Inputs: the check, the unit sent and the tidied answer. Returns the reason to refuse the answer, or None when it has the right shape.
pub fn check(check: Check, input: &str, answer: &str) -> Option<Reason> {
    let ok = match check {
        Check::None => return None,
        Check::Bullets => dash_lines(answer).then_some(()).ok_or(Reason::NotBullets),
        Check::Items => (answer.trim() == input.trim() || dash_lines(answer)).then_some(()).ok_or(Reason::NotBullets),
        Check::Shorter => {
            let words = |s: &str| s.split_whitespace().count();
            (words(answer) <= words(input)).then_some(()).ok_or(Reason::NotShorter)
        }
        Check::Summary => (answer.chars().count() <= input.chars().count()).then_some(()).ok_or(Reason::NotShorter),
        Check::Fix => {
            let flipped = has_word(input, &NEGATIONS) != has_word(answer, &NEGATIONS) || future(input) != future(answer);
            (!flipped).then_some(()).ok_or(Reason::LostNegation)
        }
    };
    ok.err()
}
