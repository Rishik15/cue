//! Purpose: Check and tidy what the model answered before it replaces the user's text, so a bad answer never overwrites good text.
//! Contents: clean — trim, unwrap quotes the model added, reject empty or runaway answers; max_tokens — the generation limit for an input.

/// Longest answer accepted, as a multiple of the input plus a margin for very short inputs. A longer answer is the model rambling, not editing.
const MAX_GROWTH: usize = 3;
const MARGIN: usize = 200;

pub fn clean(input: &str, reply: &str) -> Option<String> {
    let mut text = reply.trim();
    let quoted =
        |s: &str| s.len() > 1 && ((s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\u{201c}') && s.ends_with('\u{201d}')));
    if quoted(text) && !quoted(input.trim()) {
        text = text.trim_matches(|c| c == '"' || c == '\u{201c}' || c == '\u{201d}').trim();
    }
    let runaway = text.chars().count() > input.chars().count() * MAX_GROWTH + MARGIN;
    (!text.is_empty() && !runaway).then(|| text.to_string())
}

/// About 4 characters per token for English; twice the input leaves room for a longer rewrite without letting the model run on.
pub fn max_tokens(input: &str) -> usize {
    (input.chars().count() / 2 + 64).min(2048)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_and_unwraps_added_quotes() {
        assert_eq!(clean("hello wrold", "  \"Hello world\"\n").as_deref(), Some("Hello world"));
        assert_eq!(clean("\"hi\"", "\"Hi\"").as_deref(), Some("\"Hi\""), "quotes the user wrote stay");
    }

    #[test]
    fn rejects_empty_and_runaway_answers() {
        assert_eq!(clean("hi", "   "), None);
        assert_eq!(clean("hi", &"x".repeat(500)), None);
        assert!(clean(&"word ".repeat(100), &"word ".repeat(120)).is_some());
    }

    #[test]
    fn generation_limit_scales_and_is_capped() {
        assert_eq!(max_tokens("hi"), 65);
        assert_eq!(max_tokens(&"x".repeat(100_000)), 2048);
    }
}
