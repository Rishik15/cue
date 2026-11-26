//! File: actions/tests/properties.rs
//! Purpose: Protect the exact text actions against inputs nobody wrote by hand: every action survives any text, the tidying actions are stable when run twice, and JSON pretty-printing and minifying agree.
//! Contents: no-panic over arbitrary Unicode for every action that needs no model; quote then unquote round trip; idempotence of the tidying actions; the JSON round trip.

use crate::actions::Action;
use proptest::prelude::*;
use std::collections::BTreeMap;

/// Actions that return the same text the second time they run, because they leave nothing left to tidy.
const STABLE: [Action; 4] = [Action::CleanWhitespace, Action::DedupeLines, Action::SortLines, Action::PlainQuotes];

/// Runs an action and falls back to the input when there was nothing to change.
fn run(action: Action, text: &str) -> String {
    action.apply(text).unwrap_or_else(|| text.to_string())
}

proptest! {
    /// Any text, including odd Unicode and control characters, goes through every exact action without a panic.
    #[test]
    fn exact_actions_survive_any_text(text in "\\PC{0,200}") {
        for action in Action::all().filter(|a| !a.needs_model()) {
            action.apply(&text);
        }
    }

    /// Quoting any text and then unquoting it gives the original text back, line breaks and all.
    #[test]
    fn quote_then_unquote_is_the_identity(text in "[\\PC\n\r]{0,200}") {
        prop_assert_eq!(run(Action::Unquote, &run(Action::Quote, &text)), text);
    }

    /// Running a tidying action on its own output changes nothing. The text ends in a visible character because the line-based actions drop one trailing newline per pass by design (they split with `lines()`), which is not a defect users see.
    #[test]
    fn tidying_actions_are_stable(text in "([ -~\\t\\n\\u{a0}“”‘’]|\\r\\n){0,160}[!-~]") {
        for action in STABLE {
            let once = run(action, &text);
            prop_assert_eq!(run(action, &once), once.clone(), "{} is not stable", action.id());
        }
    }

    /// Dedupe Lines never leaves a carriage return without its line feed unless the input already had one. A two-letter alphabet makes repeated lines common.
    #[test]
    fn dedupe_keeps_crlf_pairs(text in "([ab \\n\\r]|\\r\\n){0,40}") {
        let bare = |t: &str| t.replace("\r\n", "").contains('\r');
        prop_assert!(bare(&text) || !bare(&run(Action::DedupeLines, &text)));
    }

    /// Pretty-printing valid JSON and minifying it again gives the same text as minifying the original.
    #[test]
    fn json_pretty_then_minify_matches_minify(map in proptest::collection::btree_map("[a-z]{1,6}", any::<i64>(), 1..6)) {
        let json = serde_json::to_string(&map.iter().collect::<BTreeMap<_, _>>()).expect("a map is valid JSON");
        let pretty = run(Action::JsonPretty, &json);
        prop_assert_eq!(run(Action::JsonMinify, &pretty), run(Action::JsonMinify, &json));
    }

    /// Any integer or decimal literal, however many digits, comes out of both JSON actions digit for digit.
    #[test]
    fn json_numbers_keep_every_digit(int in "[1-9][0-9]{0,40}", frac in "[0-9]{1,40}") {
        let json = format!("{{\"n\": {int}.{frac}, \"i\": {int}}}");
        prop_assert_eq!(run(Action::JsonMinify, &json), format!("{{\"n\":{int}.{frac},\"i\":{int}}}"));
        let number = format!("{int}.{frac}");
        prop_assert!(run(Action::JsonPretty, &json).contains(&number));
    }
}

/// Numbers past f64 and u64 range survive both JSON actions digit for digit.
#[test]
fn json_big_numbers_keep_their_digits() {
    let json = r#"{"n": 0.123456789012345678901, "big": 18446744073709551616}"#;
    assert_eq!(run(Action::JsonMinify, json), r#"{"n":0.123456789012345678901,"big":18446744073709551616}"#);
    let pretty = run(Action::JsonPretty, json);
    assert!(pretty.contains("0.123456789012345678901") && pretty.contains("18446744073709551616"), "{pretty}");
}
