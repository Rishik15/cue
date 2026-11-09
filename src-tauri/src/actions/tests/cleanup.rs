//! File: actions/tests/cleanup.rs
//! Purpose: Protect the everyday text jobs: tidy spacing, join PDF lines, bullets, case, identifiers, lists, JSON, fillers and spoken numbers.
//! Contents: one scenario per job, run through `Action::apply` like the palette does.

use crate::actions::Action;

/// Pasted text with ragged spacing comes back with single spaces and at most one blank line.
#[test]
fn messy_spacing_is_tidied_to_single_spaces_and_one_blank_line() {
    let out = Action::CleanWhitespace.apply("  Hello   world \n\n\n\n  second\tline  \n\n").expect("changes");
    assert_eq!(out, "Hello world\n\nsecond line");
    assert_eq!(Action::CleanWhitespace.apply("already tidy"), None);
}

/// Text wrapped by a PDF is rejoined into paragraphs: a hyphenated word is mended and a list keeps its own lines.
#[test]
fn pdf_line_breaks_are_joined_but_lists_and_paragraphs_survive() {
    let pdf = "The quick brown fox jumps over the lazy dog and the con-\ntinued story goes\non and on.\n\n- item one\n- item two";
    let out = Action::JoinLines.apply(pdf).expect("changes");
    assert_eq!(out, "The quick brown fox jumps over the lazy dog and the continued story goes on and on.\n\n- item one\n- item two");
}

/// One line of sentences becomes bullets per sentence; several lines become one bullet each, and existing markers are not doubled.
#[test]
fn text_becomes_bullets_without_doubling_existing_markers() {
    assert_eq!(
        Action::SplitBullets.apply("Buy milk. Call Sam! Send the file").as_deref(),
        Some("- Buy milk.\n- Call Sam!\n- Send the file")
    );
    assert_eq!(Action::SplitBullets.apply("1. first\n- second\nthird").as_deref(), Some("- first\n- second\n- third"));
}

/// A single numbered item is one bullet, not an empty "1." bullet and the text; a numbered item with several sentences still splits per sentence.
#[test]
fn a_single_numbered_item_becomes_one_bullet_without_its_number() {
    assert_eq!(Action::SplitBullets.apply("1. first").as_deref(), Some("- first"));
    assert_eq!(Action::SplitBullets.apply("2) Buy milk. Call Sam!").as_deref(), Some("- Buy milk.\n- Call Sam!"));
}

/// Case conversions: title, sentence, and the three identifier styles, line by line.
#[test]
fn case_and_identifier_conversions_follow_their_styles() {
    assert_eq!(Action::TitleCase.apply("the QUICK brown fox").as_deref(), Some("The Quick Brown Fox"));
    assert_eq!(Action::SentenceCase.apply("hELLO there. HOW are you? fine").as_deref(), Some("Hello there. How are you? Fine"));
    assert_eq!(Action::SnakeCase.apply("user name\nUserID").as_deref(), Some("user_name\nuser_id"));
    assert_eq!(Action::CamelCase.apply("user name\nUserID").as_deref(), Some("userName\nuserId"));
    assert_eq!(Action::KebabCase.apply("user name\nUserID").as_deref(), Some("user-name\nuser-id"));
}

/// Sorting ignores case, dropping duplicates keeps the first, and curly quotes become straight ones.
#[test]
fn lists_are_sorted_deduplicated_and_quotes_straightened() {
    assert_eq!(Action::SortLines.apply("banana\nApple\ncherry").as_deref(), Some("Apple\nbanana\ncherry"));
    assert_eq!(Action::DedupeLines.apply("a\nb\na\nc").as_deref(), Some("a\nb\nc"));
    assert_eq!(Action::PlainQuotes.apply("\u{201c}it\u{2019}s\u{201d}").as_deref(), Some("\"it's\""));
}

/// JSON is indented or squeezed to one line; text that is not JSON is never rewritten.
#[test]
fn json_is_formatted_and_minified_and_other_text_is_left_alone() {
    let pretty = Action::JsonPretty.apply("{\"a\":[1,2]}").expect("valid json");
    assert_eq!(pretty, "{\n  \"a\": [\n    1,\n    2\n  ]\n}");
    assert_eq!(Action::JsonMinify.apply(&pretty).as_deref(), Some("{\"a\":[1,2]}"));
    assert_eq!(Action::JsonPretty.apply("{not json"), None);
    assert_eq!(Action::JsonMinify.apply("hello"), None);
}

/// A dictated sentence loses its fillers and stutters without losing its capital letter or full stop.
#[test]
fn dictated_fillers_and_repeated_words_are_removed_keeping_the_sentence() {
    assert_eq!(
        Action::RemoveFillers.apply("Um, so we should, uh, ship it on friday.").as_deref(),
        Some("So we should, ship it on friday.")
    );
    assert_eq!(Action::RemoveRepeats.apply("we we should go to to the the park").as_deref(), Some("we should go to the park"));
    assert_eq!(Action::RemoveFillers.apply("nothing to remove here"), None);
}

/// Numbers said in words become digits, but a lone small number stays a word.
#[test]
fn spoken_numbers_become_digits_except_a_lone_small_one() {
    let out = Action::SpokenNumbers.apply("we need twenty five chairs and nine tables").expect("changes");
    assert_eq!(out, "we need 25 chairs and nine tables");
    let big = Action::SpokenNumbers.apply("it cost two thousand three hundred forty two dollars").expect("changes");
    assert_eq!(big, "it cost 2342 dollars");
}
