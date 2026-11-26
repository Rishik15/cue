//! File: lab/tests.rs
//! Purpose: Prove the lab surface answers exactly as production: the judge equals `accept_for` (tense, Keep::Spans, char counting), keeps the tidied text on a rejection and refuses bad input without a panic;
//! the task dump carries the guard and every model task; clean strips what Cue strips; installed follows the catalog rule (the VAD alone installs nothing).
//! Contents: judged — run judge_rows on one row; the tests.

use serde_json::{json, Value};

use super::{clean, dispatch, installed, judge, tasks};
use crate::actions::{task, Action, GUARD};
use crate::speech::catalog::{self, VAD};
use crate::text::reply::{accept_for, Reason, Verdict};

/// Judges one row through `judge_rows`. Inputs: action id, input, answer. Returns the verdict object.
fn judged(action: &str, input: &str, answer: &str) -> Value {
    let rows = json!([{ "action": action, "input": input, "answer": answer }]).to_string();
    let out: Value = serde_json::from_str(&judge::judge_rows(&rows).expect("judged")).expect("JSON out");
    out[0].clone()
}

/// "I will send it" corrected to "I sent it" loses the future: production rejects it, and so does the lab (the old PowerShell scorer accepted it).
#[test]
fn a_lost_future_is_rejected() {
    let v = judged("correct", "I will send it", "I sent it");
    assert_eq!(v["accepted"], false);
    assert_eq!(v["reason"], "lost-negation");
}

/// Tidy speech keeps only the maskable spans: dropping a bare number while keeping the link is accepted (the old scorer called it lost-protected).
#[test]
fn tidy_speech_may_drop_a_bare_number() {
    let v = judged("tidy_speech", "so um call me at 5 about https://example.com/x", "Call me about https://example.com/x.");
    assert_eq!(v["accepted"], true, "{v}");
    assert_eq!(v["reason"], Value::Null);
}

/// The change limit counts chars, not UTF-16 units: ten emoji plus seven letters is over-edited by chars and would pass by UTF-16; the lab says what `accept_for` says.
#[test]
fn supplementary_characters_count_as_one() {
    let input = "\u{1F600}".repeat(10);
    let answer = format!("{input}abcdefg");
    let correct = task(Action::Correct).expect("model task");
    let direct = accept_for(correct, &input, &answer);
    assert_eq!(direct, Verdict::Reject(Reason::OverEdited));
    let v = judged("correct", &input, &answer);
    assert_eq!(v["accepted"], false);
    assert_eq!(v["reason"], "over-edited");
}

/// A rejected answer still comes back tidied: code fence and lead-in removed.
#[test]
fn a_rejected_answer_is_still_tidied() {
    let v = judged("correct", "I will send it", "```text\nHere is the corrected text:\nI sent it.\n```");
    assert_eq!(v["accepted"], false);
    assert_eq!(v["text"], "I sent it.");
}

/// An unknown or exact action and invalid JSON are errors, not panics; an unknown command and a missing file too.
#[test]
fn bad_input_is_an_error() {
    let row = |id: &str| json!([{ "action": id, "input": "a", "answer": "b" }]).to_string();
    assert!(judge::judge_rows(&row("no_such_action")).is_err());
    assert!(judge::judge_rows(&row(Action::all().find(|a| !a.needs_model()).expect("an exact action").id())).is_err());
    assert!(judge::judge_rows("{not json").is_err());
    assert!(clean::clean_strings("[1, 2]").is_err());
    assert!(dispatch(&["nope".into()]).is_err());
    assert!(dispatch(&[]).is_err());
    assert!(dispatch(&["judge".into()]).is_err());
    assert!(dispatch(&["judge".into(), "/no/such/file.json".into()]).is_err());
}

/// The dump is JSON with the guard and every model task, and the correction prompt ends with the guard sentence.
#[test]
fn the_dump_has_the_guard_and_every_model_task() {
    let dump: Value = serde_json::from_str(&tasks::dump().expect("dump")).expect("JSON");
    assert!(!GUARD.is_empty());
    assert_eq!(dump["guard"], GUARD);
    for action in Action::all() {
        let entry = &dump["tasks"][action.id()];
        match task(action) {
            Some(t) => {
                assert_eq!(entry["system"], t.system, "{}", action.id());
                assert_eq!(entry["shots"].as_array().expect("shots").len(), t.shots.len() * 2);
            }
            None => assert!(entry.is_null(), "{} has no model task", action.id()),
        }
    }
    assert!(dump["tasks"]["correct"]["system"].as_str().expect("system").ends_with(GUARD));
}

/// Tag characters, a bidi override and a zero-width space are removed; a tab and the zero-width joiner stay.
#[test]
fn clean_strips_what_cue_strips() {
    let out = clean::clean_strings(&json!(["a\u{E0041}b\u{202E}c\u{200B}d\te\u{200D}f"]).to_string()).expect("cleaned");
    assert_eq!(out, json!(["abcd\te\u{200D}f"]).to_string());
}

/// The VAD alone installs nothing; a streaming model with every file at its catalog size is listed; one byte short is not; a missing folder is an error.
#[test]
fn installed_follows_the_catalog_rule() {
    let root = tempfile::tempdir().expect("temp dir");
    let make = |dir: &std::path::Path, name: &str, size: u64| {
        std::fs::create_dir_all(dir).expect("dir");
        std::fs::File::create(dir.join(name)).and_then(|f| f.set_len(size)).expect("file");
    };
    make(root.path(), VAD.name, VAD.size);
    assert_eq!(installed::list(root.path()).expect("listed"), "");
    let model = catalog::all().find(|m| m.is_streaming()).expect("a streaming model");
    let dir = root.path().join(model.id);
    for f in model.files {
        make(&dir, f.name, f.size);
    }
    assert_eq!(installed::list(root.path()).expect("listed"), format!("{}\n", model.id));
    let last = model.files.last().expect("files");
    make(&dir, last.name, last.size - 1);
    assert_eq!(installed::list(root.path()).expect("listed"), "");
    assert!(installed::list(&root.path().join("missing")).is_err());
}
