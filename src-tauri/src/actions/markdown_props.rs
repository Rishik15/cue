//! Purpose: Property tests for the Markdown converter: whatever HTML a program puts on the clipboard (broken, hostile, odd Unicode), converting it must never panic.
use proptest::prelude::*;

use super::markdown::to_markdown;

proptest! {
    #[test]
    fn any_text_converts_without_panicking(s in ".*") {
        let _ = to_markdown(&s);
    }

    #[test]
    fn tag_soup_converts_without_panicking(s in "[<>/a-zA-Z=\"' &;#0-9!-]{0,200}") {
        let _ = to_markdown(&s);
    }

    #[test]
    fn characters_whose_lowercase_changes_length_do_not_shift_attributes(s in "(\u{130}|\u{1E9E}|<a |<img |href=|src=|alt=|\"|>|x| ){0,40}") {
        let _ = to_markdown(&s);
    }

    #[test]
    fn output_has_no_runs_of_three_blank_lines(s in "[<>/a-z0-9 \n]{0,200}") {
        prop_assert!(!to_markdown(&s).contains("\n\n\n"));
    }
}
