//! File: dictionary/copied.rs
//! Purpose: What the user is told about a copy the dictionary used, so a lookup never leaves their clipboard lost without a word.
//! Contents: warning — the line for a copy whose clipboard was not put back; read — what a copy means to the dictionary: that line and the cleaned word.

use crate::platform::Copied;

/// The line to show when the dictionary used `copied` but the clipboard the user had before could not be put back. Input: the copy. Return: the line, None when the clipboard is back.
pub fn warning(copied: &Copied) -> Option<&'static str> {
    (!copied.restored).then_some("Cue could not put your previous clipboard back after reading the selection.")
}

/// What the dictionary takes from a copy, the warning decided before the text is cleaned so a copy that cleans to nothing still reports a lost clipboard. Input: the copy. Return: the warning line (None when the clipboard is back) and the cleaned word (empty when nothing is left, so the search window opens).
pub fn read(copied: &Copied) -> (Option<&'static str>, String) {
    (warning(copied), crate::sanitize::clean_selection(&copied.text))
}

#[cfg(test)]
mod tests {
    use super::{read, warning};
    use crate::platform::{Copied, Target};

    /// A copy from a clipboard that was not put back is reported; one that was is not.
    #[test]
    fn a_copy_that_lost_the_clipboard_is_reported() {
        let copy = |restored| Copied { text: "word".into(), html: None, restored, target: Target::default() };
        assert!(warning(&copy(false)).is_some());
        assert!(warning(&copy(true)).is_none());
    }

    /// A copy that cleans to nothing (spaces, `&nbsp;`, a hidden mark) still carries the lost-clipboard warning while the outcome is "search"; a clean word keeps both its text and its warning, and a restored one has none.
    #[test]
    fn a_copy_that_cleans_to_nothing_still_warns() {
        let copy = |text: &str, restored| Copied { text: text.into(), html: None, restored, target: Target::default() };
        assert_eq!(read(&copy(" \u{A0}&nbsp;\u{200B}", false)), (warning(&copy("", false)), String::new()));
        assert!(read(&copy(" \u{A0}&nbsp;\u{200B}", false)).0.is_some());
        assert_eq!(read(&copy(" \u{A0}&nbsp;", true)), (None, String::new()));
        assert_eq!(read(&copy(" word ", false)), (warning(&copy("", false)), "word".to_string()));
    }
}
