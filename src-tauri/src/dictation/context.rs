//! Purpose: Fit a dictated text to what is already before the cursor ("Context-Aware Paste"): a leading space when the text would
//! otherwise stick to the previous word, and a capital letter only where a sentence begins.
//! Contents: fit — the adjusted text. Pure, so it is unit tested; reading the surroundings is `platform::text_before_cursor`.
//! ponytail: a name that starts a mid-sentence dictation is lowercased ("Seattle" becomes "seattle"); a name list is the upgrade.

/// `before` is the few characters in front of the cursor ("" at the start of the field).
pub fn fit(text: &str, before: &str) -> String {
    let Some(first) = text.chars().next() else { return String::new() };
    let last = before.chars().last();
    let sticks =
        last.is_some_and(|c| !c.is_whitespace() && !"([{\"'“‘/-".contains(c)) && !first.is_whitespace() && !",.;:!?)]}".contains(first);
    let sentence_start = before.trim_end_matches(' ').is_empty()
        || before.trim_end_matches(' ').ends_with('\n')
        || before.trim_end().chars().last().is_some_and(|c| ".!?".contains(c));
    let body = if sentence_start { capitalize(text) } else { lower_first(text) };
    if sticks {
        format!(" {body}")
    } else {
        body
    }
}

fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

/// Leaves "I", "I'm" and words with another capital (NASA, iPhone) alone.
fn lower_first(text: &str) -> String {
    let word = text.split_whitespace().next().unwrap_or("");
    let keep = word == "I" || word.starts_with("I'") || word.chars().skip(1).any(char::is_uppercase);
    let mut chars = text.chars();
    match chars.next() {
        Some(c) if !keep => c.to_lowercase().chain(chars).collect(),
        _ => text.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capital_at_the_start_and_after_a_sentence() {
        assert_eq!(fit("hello there", ""), "hello there".replacen('h', "H", 1));
        assert_eq!(fit("Next thing", "ed. "), "Next thing");
        assert_eq!(fit("next thing", "ed.\n"), "Next thing");
    }

    #[test]
    fn lowercase_and_space_mid_sentence() {
        assert_eq!(fit("Then we left", "and"), " then we left");
        assert_eq!(fit("Then we left", "and "), "then we left");
        assert_eq!(fit("I think so", "and "), "I think so");
        assert_eq!(fit("NASA agrees", "and "), "NASA agrees");
    }

    #[test]
    fn no_space_before_punctuation_or_after_an_opener() {
        assert_eq!(fit(", right", "yes"), ", right");
        assert_eq!(fit("quote", "(").as_str(), "quote");
    }
}
