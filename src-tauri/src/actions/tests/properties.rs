//! File: actions/tests/properties.rs
//! Purpose: Protect the exact text actions against inputs nobody wrote by hand: every action survives any text, the tidying actions are stable when run twice, and JSON pretty-printing and minifying agree.
//! Contents: no-panic over arbitrary Unicode for every action that needs no model; idempotence of the tidying actions; the JSON round trip.

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
    /// Any text, including odd Unicode and control characters, goes through every exact action without a panic and never yields an empty replacement.
    #[test]
    fn exact_actions_survive_any_text(text in "\\PC{0,200}") {
        for action in Action::all().filter(|a| !a.needs_model()) {
            if let Some(out) = action.apply(&text) {
                prop_assert!(!out.is_empty(), "{} returned an empty replacement", action.id());
            }
        }
    }

    /// Running a tidying action on its own output changes nothing.
    #[test]
    fn tidying_actions_are_stable(text in "[ -~\\t\\n\\u{a0}“”‘’]{0,160}") {
        for action in STABLE {
            let once = run(action, &text);
            prop_assert_eq!(run(action, &once), once.clone(), "{} is not stable", action.id());
        }
    }

    /// Pretty-printing valid JSON and minifying it again gives the same text as minifying the original.
    #[test]
    fn json_pretty_then_minify_matches_minify(map in proptest::collection::btree_map("[a-z]{1,6}", any::<i32>(), 1..6)) {
        let json = serde_json::to_string(&map.iter().collect::<BTreeMap<_, _>>()).expect("a map is valid JSON");
        let pretty = run(Action::JsonPretty, &json);
        prop_assert_eq!(run(Action::JsonMinify, &pretty), run(Action::JsonMinify, &json));
    }
}
