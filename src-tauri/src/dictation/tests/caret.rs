//! File: dictation/tests/caret.rs
//! Purpose: Protect Match Surrounding Text: dictated text gets the space and the capital letter its place in the sentence needs, and nothing else.
//! Contents: scenario tests of `context::fit`.

use crate::dictation::context::fit;

/// At the start of a field or after a full stop the text begins with a capital; mid-sentence it begins in lower case.
#[test]
fn text_is_capitalised_only_where_a_sentence_begins() {
    assert_eq!(fit("hello world", ""), "Hello world");
    assert_eq!(fit("and then we left", "It rained. "), "And then we left");
    assert_eq!(fit("and then we left", "Line one\n"), "And then we left");
    assert_eq!(fit("The weather", "I think "), "the weather");
}

/// Words that need their capitals keep them mid-sentence: I, I'm, acronyms and brand spellings.
#[test]
fn pronoun_acronyms_and_brand_names_keep_their_capitals() {
    assert_eq!(fit("I agree", "he said "), "I agree");
    assert_eq!(fit("I'm here", "he said "), "I'm here");
    assert_eq!(fit("NASA launched", "we saw "), "NASA launched");
    assert_eq!(fit("iPhone sales", "the "), "iPhone sales");
}

/// A space is added when the text would stick to the previous word, and not after a space, an opening bracket or before punctuation.
#[test]
fn a_leading_space_is_added_only_when_the_text_would_stick() {
    assert_eq!(fit("hello", "Dear Sam,"), " hello");
    assert_eq!(fit("hello", "Dear Sam, "), "hello");
    assert_eq!(fit("note", "("), "note");
    assert_eq!(fit(", and so on", "words"), ", and so on");
    assert_eq!(fit("", "anything"), "");
}
