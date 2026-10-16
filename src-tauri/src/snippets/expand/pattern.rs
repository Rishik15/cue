//! Purpose: Triggers written as regular expressions: `re:\d+px` fires when the typed text ends with a match, and the groups of the match can be used in the snippet's text with `{match}` and `{match:1}`.
//! Contents: PREFIX — what marks a trigger as a pattern; compile — a pattern checked for use as a trigger (the message is shown to the user); Pattern / Captured — a compiled trigger and what it found.
//! The `regex` crate takes time linear in the input, so a pattern cannot stall typing; a pattern that could match nothing is refused, since it would fire on every key.

use regex::{Regex, RegexBuilder};

pub const PREFIX: &str = "re:";
/// Compiled size cap per pattern; the default (10 MB) lets a few hundred imported patterns exhaust memory.
const SIZE_LIMIT: usize = 64 << 10;

/// The pattern anchored to the end of the typed text.
pub fn compile(pattern: &str) -> Result<Regex, String> {
    let invalid = |_| "This pattern is not a valid regular expression.".to_string();
    // Alone first: `a)|(b` is only valid once wrapped, and then its `|` would escape the end anchor.
    RegexBuilder::new(pattern).size_limit(SIZE_LIMIT).build().map_err(invalid)?;
    let re = RegexBuilder::new(&format!("(?:{pattern})$")).size_limit(SIZE_LIMIT).build().map_err(invalid)?;
    if re.is_match("") {
        return Err("A pattern must not match an empty text.".into());
    }
    Ok(re)
}

pub struct Pattern(Regex);

/// What a pattern found: the matched text and each group by number (and by name when it has one).
pub struct Captured {
    pub text: String,
    pub groups: Vec<(String, String)>,
}

impl Pattern {
    /// None when the trigger is not a pattern or does not compile.
    pub fn new(trigger: &str) -> Option<Pattern> {
        compile(trigger.strip_prefix(PREFIX)?).ok().map(Pattern)
    }

    pub fn find(&self, typed: &str) -> Option<Captured> {
        let caps = self.0.captures(typed)?;
        let mut groups = Vec::new();
        for (index, name) in self.0.capture_names().enumerate() {
            let Some(found) = caps.get(index) else { continue };
            groups.push((index.to_string(), found.as_str().to_string()));
            groups.extend(name.map(|n| (n.to_string(), found.as_str().to_string())));
        }
        Some(Captured { text: caps.get(0)?.as_str().to_string(), groups })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pattern_matches_the_end_of_the_typed_text_and_names_its_groups() {
        let p = Pattern::new(r"re:(?P<n>\d+)px").unwrap();
        let found = p.find("width 12px").unwrap();
        assert_eq!(found.text, "12px");
        assert!(found.groups.contains(&("1".into(), "12".into())) && found.groups.contains(&("n".into(), "12".into())));
        assert!(p.find("12px more").is_none() && Pattern::new(":sig").is_none());
    }

    #[test]
    fn patterns_that_match_nothing_or_do_not_compile_are_refused() {
        assert!(compile("a*").is_err() && compile("(").is_err() && compile("a+").is_ok());
        assert!(compile("a)|(b").is_err() && compile(r"\w{300}").is_err());
    }
}
