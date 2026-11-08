//! File: dictation/tests/transcript.rs
//! Purpose: Protect the clean-up of a transcript before it is typed: filler sounds go, names come out spelled as the user wrote them, and ordinary words are never swallowed by a vocabulary entry.
//! Contents: scenario tests of `clean::clean`.

use crate::dictation::clean::clean;

const VOCABULARY: &str = "# names and terms\nPhoebe\nKubernetes\ngit hub -> GitHub\n";

/// A realistic dictated sentence: filler sounds removed, a near-miss name fixed, a term cased as saved, a spoken phrase replaced, punctuation kept.
#[test]
fn a_dictated_sentence_loses_fillers_and_gets_names_spelled_as_saved() {
    let said = "Um, Pheebe asked if kubernetes runs on git hub, hmm, today.";
    assert_eq!(clean(said, VOCABULARY, true), "Phoebe asked if Kubernetes runs on GitHub, today.");
    assert_eq!(clean(said, VOCABULARY, false), "Um, Phoebe asked if Kubernetes runs on GitHub, hmm, today.");
}

/// A filler that ends a sentence hands its full stop to the word before it, and the next sentence keeps its capital.
#[test]
fn a_filler_at_a_sentence_end_keeps_the_full_stop_and_the_next_capital() {
    assert_eq!(clean("We should go, um. then we eat", "", true), "We should go. Then we eat");
    assert_eq!(clean("hello er world", "", true), "hello world");
}

/// Short vocabulary words must match exactly: a name like Theo never turns the ordinary word "then" into a name.
#[test]
fn a_short_name_does_not_swallow_similar_ordinary_words() {
    assert_eq!(clean("Theo came then left", "Theo", true), "Theo came then left");
    assert_eq!(clean("theo came", "Theo", true), "Theo came");
}

/// An explicit replacement works on whole words in any case and on phrases, and the longer phrase wins when two overlap.
#[test]
fn explicit_replacements_match_whole_words_and_the_longest_phrase_first() {
    let vocabulary = "gonna -> going to\nnew york -> New York\nnew york city -> NYC";
    assert_eq!(clean("I am Gonna visit new york city and new york", vocabulary, false), "I am going to visit NYC and New York");
    assert_eq!(clean("gonnabe fine", vocabulary, false), "gonnabe fine");
}

/// Blank lines, comments and entries with nothing to match are ignored, and empty or silent input stays empty.
#[test]
fn comments_blank_lines_and_empty_input_are_harmless() {
    assert_eq!(clean("hello", "\n# only a comment\n  \n -> \n", true), "hello");
    assert_eq!(clean("   ", VOCABULARY, true), "");
    assert_eq!(clean("um uh hmm", "", true), "");
}
