//! Purpose: Tidy a transcript before it is typed: drop filler sounds and apply the user's vocabulary (names and terms the engine
//! tends to misspell, and explicit replacements). Pure text in, text out.
//! Contents: clean — the entry point; Rule / parse — one vocabulary line (`Name` fixes near misses, `from -> to` replaces exactly);
//! apply_rules — match words and phrases; levenshtein — edit distance.
//! Vocabulary format: one entry per line; blank lines and lines starting with # are ignored.

const FILLERS: [&str; 7] = ["uh", "um", "uhm", "umm", "er", "erm", "hmm"];

enum Rule {
    /// Whole words, any case, become `to`.
    Replace { from: Vec<String>, to: String },
    /// Words within a few edits of `shown` become `shown`, so a name always comes out spelled as the user typed it. `joined` and `allowed`
    /// are worked out once: the most edits allowed depends on the length of the name.
    Near { words: Vec<String>, shown: String, joined: String, allowed: usize },
}

impl Rule {
    fn words(&self) -> &[String] {
        match self {
            Rule::Replace { from, .. } => from,
            Rule::Near { words, .. } => words,
        }
    }

    fn output(&self) -> &str {
        match self {
            Rule::Replace { to, .. } => to,
            Rule::Near { shown, .. } => shown,
        }
    }

    /// Does the candidate text (already normalised words) stand for this rule?
    fn matches(&self, candidate: &[String]) -> bool {
        match self {
            Rule::Replace { from, .. } => from == candidate,
            Rule::Near { words, joined, allowed, .. } => {
                if words.len() != candidate.len() {
                    return false;
                }
                // Too different in length to be within the allowed edits: skip the edit distance (most words in a text land here).
                let got_len: usize = candidate.iter().map(|w| w.chars().count()).sum();
                got_len.abs_diff(joined.chars().count()) <= *allowed && levenshtein(joined, &candidate.concat()) <= *allowed
            }
        }
    }
}

/// Lower-case letters and digits only, so "Phoebe," and "phoebe" compare equal.
fn norm(word: &str) -> String {
    word.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

fn norm_words(text: &str) -> Vec<String> {
    text.split_whitespace().map(norm).filter(|w| !w.is_empty()).collect()
}

fn parse(vocabulary: &str) -> Vec<Rule> {
    let mut rules: Vec<Rule> = vocabulary
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|line| match line.split_once("->") {
            Some((from, to)) => Some(Rule::Replace { from: norm_words(from), to: to.trim().to_string() }).filter(|r| !r.words().is_empty()),
            None => near(line).filter(|r| !r.words().is_empty()),
        })
        .collect();
    rules.sort_by_key(|r| std::cmp::Reverse(r.words().len())); // longer phrases first
    rules
}

fn near(line: &str) -> Option<Rule> {
    let words = norm_words(line);
    let joined = words.concat();
    // Short words must match exactly: one edit changes "then" into "than" or "Theo", a different word.
    let allowed = match joined.chars().count() {
        0..=4 => 0,
        5..=7 => 1,
        _ => 2,
    };
    Some(Rule::Near { words, shown: line.to_string(), joined, allowed })
}
fn levenshtein(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let above = row[j + 1];
            row[j + 1] = (diagonal + usize::from(ca != *cb)).min(row[j] + 1).min(above + 1);
            diagonal = above;
        }
    }
    row[b.len()]
}

/// Punctuation stuck to the front and back of a word, and the word between: `("(", "Phoebe", ".")`.
fn split_token(token: &str) -> (&str, &str, &str) {
    let start = token.find(|c: char| c.is_alphanumeric()).unwrap_or(token.len());
    let end = token.rfind(|c: char| c.is_alphanumeric()).map_or(start, |i| i + token[i..].chars().next().map_or(1, char::len_utf8));
    (&token[..start], &token[start..end], &token[end..])
}

fn apply_rules(tokens: &[&str], rules: &[Rule]) -> Vec<String> {
    // Each token is normalised once, not once per rule and position: a long dictation with a long vocabulary would otherwise allocate millions of strings.
    let normed: Vec<String> = tokens.iter().map(|t| norm(split_token(t).1)).collect();
    let mut out = Vec::with_capacity(tokens.len());
    let mut i = 0;
    while i < tokens.len() {
        let hit = rules.iter().find_map(|rule| {
            let n = rule.words().len();
            let window = normed.get(i..i + n)?;
            rule.matches(window).then_some((n, rule.output()))
        });
        match hit {
            Some((n, text)) => {
                out.push(format!("{}{}{}", split_token(tokens[i]).0, text, split_token(tokens[i + n - 1]).2));
                i += n;
            }
            None => {
                out.push(tokens[i].to_string());
                i += 1;
            }
        }
    }
    out
}

fn is_filler(token: &str) -> bool {
    let core = norm(split_token(token).1);
    FILLERS.contains(&core.as_str())
}

fn ends_sentence(word: &str) -> bool {
    word.ends_with(['.', '?', '!'])
}

fn upper_first(word: &str) -> String {
    let mut chars = word.chars();
    chars.next().map_or(String::new(), |c| c.to_uppercase().chain(chars).collect())
}

/// Removes filler sounds without damaging the sentence around them: a filler that closed a sentence ("we should go, um.") hands its
/// full stop to the word before it, and the word after a filler that opened or closed a sentence gets its capital.
fn drop_fillers(tokens: &[&str]) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(tokens.len());
    let mut capitalise = false;
    for token in tokens {
        if !is_filler(token) {
            out.push(if std::mem::take(&mut capitalise) { upper_first(token) } else { token.to_string() });
            continue;
        }
        let stop = split_token(token).2.chars().find(|c| ".?!".contains(*c));
        capitalise |= stop.is_some() || out.last().is_none_or(|w| ends_sentence(w));
        let (Some(stop), Some(last)) = (stop, out.last_mut()) else { continue };
        if !ends_sentence(last) {
            last.truncate(last.trim_end_matches([',', ';', ':']).len());
            last.push(stop);
        }
    }
    out
}

pub fn clean(text: &str, vocabulary: &str, remove_fillers: bool) -> String {
    let tokens: Vec<&str> = text.split_whitespace().collect();
    let kept: Vec<String> = if remove_fillers { drop_fillers(&tokens) } else { tokens.iter().map(|t| t.to_string()).collect() };
    let kept: Vec<&str> = kept.iter().map(String::as_str).collect();
    apply_rules(&kept, &parse(vocabulary)).join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drops_fillers_and_keeps_the_sentence_start_capital() {
        assert_eq!(clean("Um, I think, uh, we should go.", "", true), "I think, we should go.");
        assert_eq!(clean("Um, we should go.", "", true), "We should go.");
        assert_eq!(clean("Um, we should go.", "", false), "Um, we should go.");
    }

    #[test]
    fn a_filler_keeps_the_punctuation_and_capitals_around_it() {
        assert_eq!(clean("Okay. Um, so we go.", "", true), "Okay. So we go.");
        assert_eq!(clean("We should go, um.", "", true), "We should go.");
        assert_eq!(clean("Is it ready, uh?", "", true), "Is it ready?");
        assert_eq!(clean("Fine. Uh. Next.", "", true), "Fine. Next.");
    }

    #[test]
    fn fixes_near_misses_of_names_and_keeps_punctuation() {
        assert_eq!(clean("Tell Fibi about it.", "Phoebe", true), "Tell Fibi about it."); // too far to guess
        assert_eq!(clean("Tell Phobe about it.", "Phoebe", true), "Tell Phoebe about it.");
        assert_eq!(clean("Thanks, (kubernetes).", "Kubernetes", true), "Thanks, (Kubernetes).");
    }

    #[test]
    fn short_words_are_never_guessed() {
        assert_eq!(clean("then than", "Theo", true), "then than");
        assert_eq!(clean("then", "than", true), "then");
    }

    #[test]
    fn phrases_and_explicit_replacements() {
        assert_eq!(clean("I work at cue lab now", "Cue Labs", true), "I work at Cue Labs now");
        assert_eq!(
            clean("my email is rishik at example dot com", "rishik at example dot com -> rishik@example.com", true),
            "my email is rishik@example.com"
        );
        assert_eq!(clean("gonna go", "# comment\n\ngonna -> going to", true), "going to go");
    }

    #[test]
    fn handles_empty_input() {
        assert_eq!(clean("", "Phoebe", true), "");
        assert_eq!(clean("um", "", true), "");
    }
}
