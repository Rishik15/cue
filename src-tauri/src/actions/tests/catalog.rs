//! File: actions/tests/catalog.rs
//! Purpose: Protect the contract of the action list: the names rules.json stores, where each action is offered, and that model actions never run without a text model.
//! Contents: tests of ids, scopes and the no-model behavior.

use std::collections::HashSet;

use crate::actions::{Action, Section, Source};

/// Every action has a distinct stable id that reads back as the same action, so a rules.json written by one build loads in the next.
#[test]
fn every_action_keeps_a_unique_id_that_reads_back_from_rules_json() {
    let mut seen = HashSet::new();
    for action in Action::all() {
        let id = action.id();
        assert!(!id.is_empty(), "{action:?} has no id");
        assert!(seen.insert(id.clone()), "duplicate id {id}");
        let back: Action = serde_json::from_value(serde_json::json!(id)).expect("id parses");
        assert_eq!(back, action);
        assert!(!action.label().is_empty() && !action.hint().is_empty());
    }
}

/// Links, emails and broken lines never occur in speech, so the dictation rule editor must not offer them; the cleanups that fit speech stay.
#[test]
fn dictation_is_offered_speech_cleanups_and_not_link_or_layout_jobs() {
    for out in [Action::CleanLinks, Action::JoinLines, Action::ExtractEmails, Action::ExtractUrls, Action::SortLines] {
        assert!(!out.for_dictation(), "{out:?} should not be offered after dictation");
    }
    for fits in [Action::RemoveFillers, Action::SpokenNumbers, Action::CleanWhitespace, Action::Correct] {
        assert!(fits.for_dictation(), "{fits:?} should be offered after dictation");
    }
}

/// Text read from the screen is offered layout fixes but not spoken-text fixes; selected text gets both.
#[test]
fn screen_text_and_selected_text_are_offered_the_right_actions() {
    assert!(Action::JoinLines.offered_in(Source::Capture));
    assert!(!Action::RemoveFillers.offered_in(Source::Capture));
    assert!(Action::RemoveFillers.offered_in(Source::Selection));
    assert!(Action::Summarize.offered_in(Source::Selection));
}

/// The text model is slow and optional: every AI action is flagged so previews skip it, and with no model loaded it changes nothing instead of failing.
#[test]
fn model_actions_are_flagged_and_do_nothing_without_a_text_model() {
    for action in Action::all() {
        assert_eq!(action.needs_model(), action.section() == Section::Ai, "{action:?}");
        if action.needs_model() {
            assert_eq!(action.apply("fix this sentance please"), None, "{action:?} ran with no model");
        }
    }
}

/// Running any action on nothing is a no-op, so an empty selection or silent dictation is never written back.
#[test]
fn no_action_changes_an_empty_text() {
    for action in Action::all() {
        assert_eq!(action.apply(""), None, "{action:?}");
    }
}
