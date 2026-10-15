//! Purpose: Property tests for dictation cleanup: any transcript, any vocabulary text and either filler setting must come out without a panic, and cleaning is stable.
use proptest::prelude::*;

use super::clean::clean;

proptest! {
    #[test]
    fn any_transcript_and_vocabulary_clean_without_panicking(text in ".{0,300}", vocabulary in ".{0,300}", fillers in any::<bool>()) {
        let _ = clean(&text, &vocabulary, fillers);
    }

    #[test]
    fn vocabulary_lines_with_odd_separators_do_not_panic(text in "[a-zA-Z ,.]{0,100}", vocabulary in "[a-zA-Z =>\n,:;-]{0,200}") {
        let _ = clean(&text, &vocabulary, true);
    }
}
