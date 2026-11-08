//! File: fixups.rs
//! Purpose: Repairs the mistakes Windows' reader makes again and again, which no amount of enlarging fixes: letters that look alike in sans-serif type, and spaces it adds before punctuation.
//! Contents: fix — the whole text; fix_word — one word (capital I read as l, digits read as letters); tighten — drops a space before a comma, semicolon or full stop that follows a letter.
//! Only patterns that are almost never right as read are touched. What needs a dictionary (rn for m, cl for d) is left alone, as are `|`, camelCase and code.

/// The text with look-alike letters and digits repaired word by word, and spaces before punctuation removed.
pub fn fix(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut word = String::new();
    for c in text.chars() {
        if c.is_alphanumeric() {
            word.push(c);
            continue;
        }
        out.push_str(&fix_word(&word));
        word.clear();
        out.push(c);
    }
    out.push_str(&fix_word(&word));
    tighten(&out)
}

/// A capital I and a lowercase l are the same bar in most sans-serif fonts, and so are 1 and l, 0 and o. Two or more capitals around a lone lowercase l can only be an I (MPl, lBM); a digit with a lowercase letter on both sides, in a word that is otherwise lowercase, is a letter (Profi1e, c0de).
fn fix_word(word: &str) -> String {
    let chars: Vec<char> = word.chars().collect();
    let letters: Vec<char> = chars.iter().copied().filter(|c| c.is_alphabetic()).collect();
    let others: Vec<&char> = letters.iter().filter(|c| **c != 'l').collect();
    if letters.len() > others.len() && others.len() >= 2 && others.iter().all(|c| c.is_uppercase()) {
        return word.replace('l', "I");
    }
    let lower_rest = letters.iter().skip(1).all(|c| c.is_lowercase());
    if letters.len() < 3 || !lower_rest || !chars.iter().any(char::is_ascii_digit) {
        return word.to_string();
    }
    let is_letter = |i: usize| chars.get(i).is_some_and(|c| c.is_lowercase());
    chars
        .iter()
        .enumerate()
        .map(|(i, &c)| match c {
            '1' if i > 0 && is_letter(i - 1) && is_letter(i + 1) => 'l',
            '0' if i > 0 && is_letter(i - 1) && is_letter(i + 1) => 'o',
            _ => c,
        })
        .collect()
}

/// "one , two ." becomes "one, two."; a full stop that starts a word (.NET) or sits between digits is not touched.
fn tighten(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    for (i, &c) in chars.iter().enumerate() {
        let before_mark = c == ' ' && chars.get(i + 1).is_some_and(|m| matches!(m, ',' | ';' | '.'));
        let after_letter = out.chars().next_back().is_some_and(char::is_alphabetic);
        let ends_word = chars.get(i + 2).is_none_or(|n| n.is_whitespace());
        if !(before_mark && after_letter && ends_word) {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::fix;

    /// The repairs fire on the look-alike patterns they are written for.
    #[test]
    fn look_alike_letters_are_repaired() {
        assert_eq!(fix("MPl"), "MPI");
        assert_eq!(fix("Profi1e"), "Profile");
        assert_eq!(fix("one , two ."), "one, two.");
    }

    /// The same repairs also rewrite words that were right as read, which is why they are offered beside the raw text and never replace it: `HCl` (hydrochloric acid) becomes `HCI`, and `c0de` becomes `code`.
    #[test]
    fn the_repairs_can_change_text_that_was_right() {
        assert_eq!(fix("HCl"), "HCI");
        assert_eq!(fix("c0de"), "code");
        assert_eq!(fix("sha1sum"), "shalsum");
    }
}
