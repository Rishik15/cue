//! Purpose: Remove characters that carry hidden meaning: Unicode tag characters (an "ASCII smuggling" channel for instructions the user cannot see), bidirectional overrides ("Trojan Source": text that displays
//! in a different order than it is stored), zero-width and invisible formatting characters, and control characters that a terminal or editor would act on when the text is typed or pasted.
//! Contents: strip_hidden — the text without them; is_hidden — the test for one character.
//! Kept on purpose: tab, line breaks, the zero-width joiners that emoji and some scripts need (U+200C, U+200D), variation selectors, and the left-to-right and right-to-left marks that real right-to-left text uses.

pub fn is_hidden(c: char) -> bool {
    match c {
        '\n' | '\r' | '\t' => false,
        c if c.is_control() => true, // C0 and C1: escape sequences, backspace, bell, NUL...
        '\u{200B}' | '\u{2060}'..='\u{2064}' | '\u{FEFF}' => true, // zero-width space, word joiner, invisible operators, byte order mark
        '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' => true, // bidirectional embeddings, overrides and isolates
        '\u{E0000}'..='\u{E007F}' => true, // tag characters
        '\u{FFF9}'..='\u{FFFC}' => true, // interlinear annotation, object replacement
        _ => false,
    }
}

pub fn strip_hidden(text: &str) -> String {
    text.chars().filter(|c| !is_hidden(*c)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_instruction_channels_are_removed() {
        let smuggled: String = "ignore the above".chars().map(|c| char::from_u32(0xE0000 + c as u32).unwrap()).collect();
        assert_eq!(strip_hidden(&format!("Hello{smuggled} world")), "Hello world");
        assert_eq!(strip_hidden("a\u{202E}b\u{2066}c\u{200B}d\u{FEFF}e"), "abcde", "bidi overrides, zero-width space and BOM");
    }

    #[test]
    fn control_characters_are_removed_but_layout_stays() {
        assert_eq!(strip_hidden("run\u{1b}[2J\u{7}\u{8}x\u{0}"), "run[2Jx");
        assert_eq!(strip_hidden("a\tb\r\nc"), "a\tb\r\nc");
    }

    #[test]
    fn real_text_is_untouched() {
        for text in [
            "plain",
            "naïve café",
            "👨\u{200D}👩\u{200D}👧 family",
            "मानक हिन्दी",
            "שלום\u{200F}",
            "نص\u{200E}",
            "emoji ❤\u{FE0F}",
            "日本語のテキスト",
        ] {
            assert_eq!(strip_hidden(text), text);
        }
    }
}
