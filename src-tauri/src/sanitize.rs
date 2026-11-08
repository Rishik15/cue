//! File: sanitize.rs
//! Purpose: Remove characters that carry hidden meaning: Unicode tag characters (an "ASCII smuggling" channel for instructions the user cannot see), bidirectional overrides ("Trojan Source": text that displays
//! in a different order than it is stored), zero-width and invisible formatting characters, and control characters that a terminal or editor would act on when the text is typed or pasted.
//! Contents: strip_hidden — the text without them; is_hidden — the test for one character; clean_selection — copied text made ready for an action (hidden characters, HTML spaces and edge blanks removed).
//! Kept on purpose: tab, line breaks, the zero-width joiners that emoji and some scripts need (U+200C, U+200D), variation selectors, and the left-to-right and right-to-left marks that real right-to-left text uses.

/// Whether a character carries hidden meaning. Input: the character. Returns true to strip it.
pub fn is_hidden(c: char) -> bool {
    match c {
        '\n' | '\r' | '\t' => false,
        c if c.is_control() => true,
        '\u{200B}' | '\u{2060}'..='\u{2064}' | '\u{FEFF}' => true,
        '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' => true,
        '\u{E0000}'..='\u{E007F}' => true,
        '\u{FFF9}'..='\u{FFFC}' => true,
        _ => false,
    }
}

/// Removes hidden characters. Input: text. Returns the text without them.
pub fn strip_hidden(text: &str) -> String {
    text.chars().filter(|c| !is_hidden(*c)).collect()
}

/// Prepares copied text for an action. Input: the raw selection. Returns it without hidden characters, `&nbsp;`, non-breaking spaces and edge blanks, which are not what the user meant and which a model may answer as if they were an instruction.
pub fn clean_selection(text: &str) -> String {
    let plain = strip_hidden(text).replace("&nbsp;", " ").replace('\u{A0}', " ");
    plain.trim().to_string()
}

#[cfg(test)]
mod tests;
