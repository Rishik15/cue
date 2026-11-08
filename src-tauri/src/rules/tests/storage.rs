//! File: rules/tests/storage.rs
//! Purpose: Protect rules.json: its readable format, the rule that a damaged file is kept aside and never overwritten, and the run history that holds no text.
//! Contents: tests of the file format, `set_aside` and the run log.

use std::fs;

use super::helpers::rule;
use crate::actions::Action;
use crate::rules::model::{Match, Output, Trigger};
use crate::rules::{log, set_aside, File, VERSION};

/// A minimal hand-written file loads with the documented defaults, and a file written by Cue reads back as the same rules.
#[test]
fn rules_json_loads_with_defaults_and_round_trips() {
    let text = r#"{"version":1,"rules":[{"name":"Tidy","trigger":"dictation","actions":["clean_whitespace","remove_fillers"]}]}"#;
    let file: File = serde_json::from_str(text).expect("minimal file loads");
    let loaded = &file.rules[0];
    assert!(loaded.enabled && !loaded.stop);
    assert_eq!((loaded.when, loaded.output, loaded.trigger), (Match::All, Output::Insert, Trigger::Dictation));
    assert!(loaded.apps.is_empty() && loaded.conditions.is_empty());

    let mine = File { version: VERSION, rules: vec![rule("Shout", &[Action::UpperCase, Action::JsonMinify])] };
    let again: File = serde_json::from_str(&serde_json::to_string_pretty(&mine).expect("writes")).expect("reads");
    assert_eq!(again.rules, mine.rules);
}

/// A trigger or action name Cue does not know makes the file invalid (it is set aside, not guessed at).
#[test]
fn unknown_trigger_or_action_names_make_the_file_invalid() {
    let weekly = r#"{"version":1,"rules":[{"name":"x","trigger":"weekly","actions":["upper_case"]}]}"#;
    let invented = r#"{"version":1,"rules":[{"name":"x","trigger":"dictation","actions":["make_it_so"]}]}"#;
    assert!(serde_json::from_str::<File>(weekly).is_err());
    assert!(serde_json::from_str::<File>(invented).is_err());
}

/// A damaged rules.json is moved to rules.damaged.json with its bytes intact, so the next save from the Rules page cannot destroy what the user wrote.
#[test]
fn a_damaged_file_is_kept_aside_with_its_contents() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("rules.json");
    fs::write(&path, "{\"version\":1,\"rules\":[ half a file").expect("write");
    let Err(cause) = serde_json::from_str::<File>("{") else { panic!("half a file parsed") };
    let message = set_aside(&path, &cause);
    assert!(message.starts_with("rules.json is not valid"), "{message}");
    assert!(message.contains("rules.damaged.json"), "{message}");
    assert!(!path.exists());
    assert_eq!(fs::read_to_string(dir.path().join("rules.damaged.json")).expect("kept"), "{\"version\":1,\"rules\":[ half a file");
}

/// The run history keeps the newest 30 runs, newest first, and holds rule names only.
#[test]
fn the_run_history_keeps_thirty_newest_first_without_text() {
    for i in 0..35 {
        log::record(Trigger::Dictation, &[format!("run-{i}")], "notepad".into());
    }
    let recent = log::recent();
    assert_eq!(recent.len(), 30);
    assert_eq!(recent[0].matched, ["run-34"]);
    assert_eq!(recent[29].matched, ["run-5"]);
    assert_eq!(recent[0].app, "notepad");
}
