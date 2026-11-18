//! File: palette/tests.rs
//! Purpose: Protect what the palette offers: no row whose result is empty, because the strip and Grab cannot paste nothing.
//! Contents: tests of `builtin` and `saved_result`.

use super::{builtin, saved_result};
use crate::actions::{Action, Source};
use crate::rules::model::Output;
use crate::rules::Outcome;

/// Unquote on a lone ">" would give an empty result, so it is not offered; on text that keeps something it is.
#[test]
fn an_action_with_an_empty_result_is_not_offered() {
    let offered = |text: &str| builtin(text, false, Source::Selection).iter().any(|i| i.id == format!("action:{}", Action::Unquote.id()));
    assert!(!offered(">"));
    assert!(!offered("> \n>"));
    assert!(offered("> one"));
}

/// A saved action whose result is empty or blank (an Unquote on a lone ">") is no row; one that changed the text is.
#[test]
fn a_saved_action_with_an_empty_result_is_not_a_row() {
    let ran = |text: &str| Outcome { text: text.into(), output: Output::Insert, matched: vec!["Unquote".into()], skipped: false };
    assert_eq!(saved_result(&ran(""), ">"), None);
    assert_eq!(saved_result(&ran(" \n"), "> \n>"), None);
    assert_eq!(saved_result(&ran("one"), "> one"), Some("one".into()));
}
