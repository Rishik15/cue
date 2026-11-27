//! File: rules/tests/storage.rs
//! Purpose: Protect rules.json: its readable format, the rule that a damaged file is kept aside and never overwritten, and the run history that holds no text.
//! Contents: tests of the file format, the version match, `set_aside`, saving and the run log; json_read_time, an ignored read-time probe.

use std::fs;

use super::helpers::rule;
use crate::actions::Action;
use crate::rules::log;
use crate::rules::model::{Condition, Rule};
use crate::rules::model::{Match, Output, Trigger};
use crate::rules::storage::{decode, save_to, set_aside, File, Unusable, VERSION};

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

/// A second damaged file never replaces the first recovery copy: it goes to the next free name.
#[test]
fn a_second_damaged_file_keeps_the_first_copy() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("rules.json");
    let Err(cause) = serde_json::from_str::<File>("{") else { panic!("half a file parsed") };
    for (n, body) in ["first", "second", "third"].into_iter().enumerate() {
        fs::write(&path, body).expect("write");
        let message = set_aside(&path, &cause);
        let name = if n == 0 { "rules.damaged.json".to_string() } else { format!("rules.damaged-{}.json", n + 1) };
        assert!(message.contains(&name), "{message}");
    }
    let kept = |name: &str| fs::read_to_string(dir.path().join(name)).expect("kept");
    assert_eq!(
        (kept("rules.damaged.json"), kept("rules.damaged-2.json"), kept("rules.damaged-3.json")),
        ("first".into(), "second".into(), "third".into())
    );
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

/// A save never replaces a rules.json it cannot read as rules (a newer Cue's file, a damaged one), and refuses rules larger than Cue reads back, so no save loses data or writes a file the next start would refuse. A good save is readable again and publishes the compiled rules once.
#[test]
fn a_save_keeps_an_unreadable_file_and_refuses_one_too_large_to_read_back() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("rules.json");
    let mine = || vec![rule("Shout", &[Action::UpperCase])];
    let newer = r#"{"version":2,"rules":[]}"#;
    fs::write(&path, newer).expect("write");
    assert!(save_to(&path, mine(), |_| panic!("published")).expect_err("newer file kept").contains("newer Cue"));
    assert_eq!(fs::read_to_string(&path).expect("read"), newer);
    fs::write(&path, "{ half").expect("write");
    assert!(save_to(&path, mine(), |_| panic!("published")).is_err());
    assert_eq!(fs::read_to_string(&path).expect("read"), "{ half");

    fs::remove_file(&path).expect("remove");
    let mut published = 0;
    save_to(&path, mine(), |c| published += c.len()).expect("a missing file is written");
    assert_eq!(published, 1);
    let again: File = serde_json::from_str(&fs::read_to_string(&path).expect("read")).expect("reads back");
    assert_eq!(again.rules, mine());
    assert!(!dir.path().join("rules.json.tmp").exists());

    let big: Vec<Rule> = (0..100).map(|i| rule(&format!("Big {i}"), &[Action::UpperCase])).collect();
    let mut huge = big;
    for r in &mut huge {
        r.conditions = vec![Condition::Contains { value: "x".repeat(500) }; 30];
    }
    assert!(save_to(&path, huge, |_| panic!("published")).expect_err("too large").contains("too large"));
    let still: File = serde_json::from_str(&fs::read_to_string(&path).expect("read")).expect("old file intact");
    assert_eq!(still.rules, mine());
}

/// A file is read by its version before its shape: version 1 loads, a higher version is "newer" even when its rules look like nothing this build knows, and a lower version, a file with no version or broken text is damaged.
#[test]
fn the_version_decides_how_a_file_is_read() {
    let v1 = r#"{"version":1,"rules":[{"name":"x","trigger":"dictation","actions":["upper_case"]}]}"#;
    assert_eq!(decode(v1).ok().map(|r| r.len()), Some(1));
    let future = r#"{"version":2,"rules":[{"label":"x","when_spoken":true}]}"#;
    assert!(matches!(decode(future), Err(Unusable::Newer)));
    let ancient = r#"{"version":0,"rules":[]}"#;
    assert!(matches!(decode(ancient), Err(Unusable::Damaged(_))));
    assert!(matches!(decode(r#"{"rules":[]}"#), Err(Unusable::Damaged(_))));
    assert!(matches!(decode("{ half"), Err(Unusable::Damaged(_))));
    assert!(matches!(decode(r#"{"version":1,"rules":[{"name":"x"}]}"#), Err(Unusable::Damaged(_))));
}

/// A file of a version no Cue wrote is damaged: a save does not replace it, and the damaged path sets it aside as rules.damaged.json with its bytes intact.
#[test]
fn a_file_of_an_unknown_older_version_takes_the_damaged_path() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("rules.json");
    let old = r#"{"version":0,"rules":[]}"#;
    fs::write(&path, old).expect("write");
    let why = save_to(&path, vec![rule("Shout", &[Action::UpperCase])], |_| panic!("published")).expect_err("kept");
    assert!(why.contains("not valid"), "{why}");
    assert_eq!(fs::read_to_string(&path).expect("read"), old);
    let Err(Unusable::Damaged(cause)) = decode(old) else { panic!("version 0 is not damaged") };
    assert!(set_aside(&path, &cause).contains("rules.damaged.json"));
    assert!(!path.exists());
    assert_eq!(fs::read_to_string(dir.path().join("rules.damaged.json")).expect("kept"), old);
}

/// Read-time probe, not a check: decodes a 20-rule rules.json and parses a 40-key settings object 20,000 times each and prints the microseconds per read, so serde_json's `arbitrary_precision` feature (Cargo.toml) can be weighed. Run it as is and with the feature removed: `cargo test --release --lib json_read_time -- --ignored --nocapture`.
#[test]
#[ignore]
fn json_read_time() {
    let mine =
        File { version: VERSION, rules: (0..20).map(|i| rule(&format!("Rule {i}"), &[Action::UpperCase, Action::JsonMinify])).collect() };
    let rules = serde_json::to_string_pretty(&mine).expect("writes");
    let settings =
        serde_json::to_string(&(0..40).map(|i| (format!("key_{i}"), serde_json::json!(i * 1000))).collect::<serde_json::Map<_, _>>())
            .expect("writes");
    let runs = 20_000u32;
    let start = std::time::Instant::now();
    for _ in 0..runs {
        assert!(decode(std::hint::black_box(&rules)).is_ok());
    }
    let rules_us = start.elapsed().as_secs_f64() * 1e6 / f64::from(runs);
    let start = std::time::Instant::now();
    for _ in 0..runs {
        assert!(serde_json::from_slice::<serde_json::Value>(std::hint::black_box(settings.as_bytes())).is_ok());
    }
    let settings_us = start.elapsed().as_secs_f64() * 1e6 / f64::from(runs);
    println!("json_read_time: rules.json {rules_us:.2} us, settings.json {settings_us:.2} us per read");
}
