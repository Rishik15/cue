//! Purpose: Stitching the text of audio that was cut in the middle of speech, where the model saw each half alone: pick the words of the shared
//! stretch from one side only, drop a word both sides heard, and make the second half carry on the sentence instead of starting a new one.
//! Pure string work, so it is tested without a model.
//! Contents: words_in — the words of a decoded piece that start inside a time range; stitch — join a piece to the text before it (drop a word
//! both sides heard, lower-case a common first word, drop the full stop of a cut-off fragment).

/// Words that are lower case in the middle of a sentence and never need a capital there. A name ("Paris") is left alone: lowering it would be
/// worse than leaving an extra capital.
const LOWER: &[&str] = &[
    "the", "a", "an", "and", "or", "but", "so", "to", "of", "in", "on", "at", "for", "with", "from", "by", "as", "is", "are", "was",
    "were", "be", "it", "its", "this", "that", "these", "those", "then", "because", "which", "who", "if", "when", "while", "my", "your",
    "his", "her", "we", "you", "they", "he", "she", "not", "no", "just", "also", "very", "about", "into", "than", "what", "how", "gonna",
    "wanna", "there", "their", "our", "can", "will", "would", "could", "should", "do", "does", "did", "have", "has", "had", "all", "some",
    "any", "more", "now", "still", "even", "only", "really", "like", "well", "yeah", "maybe",
];

/// Letters and digits only, lower case: "Four," and "four" are the same word.
fn plain(word: &str) -> String {
    word.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

/// The words of a decoded piece whose first token starts between `from_s` and `to_s` seconds into it. A token with a leading space starts a
/// word (that is how the model marks it); the others (the rest of the word, punctuation) belong to the word before.
pub fn words_in(tokens: &[String], times: &[f32], from_s: f32, to_s: f32) -> String {
    let (mut out, mut keep) = (String::new(), false);
    for (i, (token, t)) in tokens.iter().zip(times).enumerate() {
        if i == 0 || token.starts_with(' ') {
            keep = (from_s..to_s).contains(t);
            if keep && !out.is_empty() {
                out.push(' ');
            }
        }
        if keep {
            out.push_str(token.trim_start());
        }
    }
    out
}

/// `text` without its first word when that is the word `previous` ended with: both sides of a cut heard it.
fn without_repeat<'a>(text: &'a str, previous: &str) -> &'a str {
    let first = text.split_whitespace().next().unwrap_or("");
    let last = previous.split_whitespace().next_back().unwrap_or("");
    match !plain(first).is_empty() && plain(first) == plain(last) {
        true => text.trim_start().strip_prefix(first).unwrap_or(text).trim_start(),
        false => text,
    }
}

/// A fragment that stops because the audio was cut is not a finished sentence, whatever its last sound suggested.
fn open_end(text: &str) -> &str {
    text.trim_end_matches(['.', '?', '!', '…']).trim_end()
}

/// The first word with a small letter when it is a common word that has no business starting a sentence by itself.
fn lowercase_start(text: &str) -> String {
    let first = text.split_whitespace().next().unwrap_or("");
    let word = first.trim_matches(|c: char| !c.is_alphanumeric());
    let mut chars = text.chars();
    match (word != "I" && LOWER.contains(&plain(word).as_str()), chars.next()) {
        (true, Some(c)) => c.to_lowercase().chain(chars).collect(),
        _ => text.to_string(),
    }
}

/// A decoded piece as it joins the text before it: a word both sides heard once, a continuing sentence in lower case (`carry_on`), and no
/// sentence end when the piece was cut (`open`).
pub fn stitch(text: &str, previous: &str, carry_on: bool, open: bool) -> String {
    let mut text = without_repeat(text, previous).to_string();
    if carry_on && !text.is_empty() {
        text = lowercase_start(&text);
    }
    match open {
        true => open_end(&text).to_string(),
        false => text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens(t: &[&str]) -> Vec<String> {
        t.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn only_words_that_start_in_the_range_are_kept_whole() {
        let t = tokens(&[" I", "'m", " gon", "na", " keep", " going", "."]);
        let times = [0.0, 0.1, 0.5, 0.6, 1.0, 1.4, 1.8];
        assert_eq!(words_in(&t, &times, 0.4, 1.2), "gonna keep");
        assert_eq!(words_in(&t, &times, 0.0, 9.0), "I'm gonna keep going.");
        assert_eq!(words_in(&t, &times, 5.0, 9.0), "");
    }

    #[test]
    fn a_word_both_sides_heard_is_not_typed_twice() {
        assert_eq!(without_repeat("the sentence I'm gonna", "after I finish the"), "sentence I'm gonna");
        assert_eq!(without_repeat("Sentence, I'm", "finish the sentence"), "I'm");
        assert_eq!(without_repeat("sentence two", "something else"), "sentence two");
        assert_eq!(without_repeat("", "x"), "");
    }

    #[test]
    fn a_cut_in_running_speech_reads_as_one_sentence() {
        let first = stitch("after I finish the sentence.", "", false, true);
        let second = stitch("Sentence, gonna keep going?", &first, true, false);
        assert_eq!(format!("{first} {second}"), "after I finish the sentence gonna keep going?");
        // the repeated word is the first side's
    }

    #[test]
    fn a_cut_fragment_does_not_end_a_sentence() {
        assert_eq!(open_end("after I finish the."), "after I finish the");
        assert_eq!(open_end("is that right?"), "is that right");
        assert_eq!(open_end("no punctuation"), "no punctuation");
    }

    #[test]
    fn the_second_half_carries_on_in_lower_case() {
        assert_eq!(lowercase_start("Gonna keep going"), "gonna keep going");
        assert_eq!(lowercase_start("The next part"), "the next part");
        assert_eq!(lowercase_start("Paris is nice"), "Paris is nice"); // a name keeps its capital
        assert_eq!(lowercase_start("I think so"), "I think so");
        assert_eq!(lowercase_start("\"We\" said"), "\"We\" said".to_string()); // quotes in front: left alone
    }
}
