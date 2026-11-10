//! File: text/tests/checks.rs
//! Purpose: Protect the per-action answer checks and the protected-span masking: each verifier accepts the right shape and refuses the wrong one with its reason, the span finder covers Windows paths, commands and amounts, and a masked span comes back exactly once or the answer is refused.
//! Contents: verifier tests (one accept and one refusal each, reasons named), tests of what the shared check demands per task, span finder tests, mask lift and restore tests, and the masked pipeline run with a stand-in model.

use crate::actions::{task, Action, Check, Keep, Scope, Task, Unit};
use crate::text::cancel::Cancel;
use crate::text::mask;
use crate::text::pipeline::{Outcome, Pipeline};
use crate::text::reply::{accept_for, Reason, Verdict};
use crate::text::spans::{MASKABLE, PROTECTED};
use crate::text::verify;

use super::scripted::scripted;

/// A task that masks, for the pipeline tests: nothing but the mask differs from a plain editing task.
static MASKED: Task = Task {
    system: "Edit.",
    shots: &[],
    unit: Unit::Sentence,
    max_change: None,
    scope: Scope::Units,
    keep: Keep::All,
    check: Check::None,
    mask: true,
};

/// What the check of `kind` says about `answer` to `input`.
fn verdict(kind: Check, input: &str, answer: &str) -> Option<Reason> {
    verify::check(kind, input, answer)
}

/// The spans of `text` that `MASKABLE` finds, in order.
fn spans(text: &str) -> Vec<&str> {
    MASKABLE.find_iter(text).map(|m| m.as_str()).collect()
}

/// Bullets must be dash lines, all of them: prose, a mixed list and an empty answer are refused.
#[test]
fn bullets_must_be_dash_lines() {
    assert_eq!(verdict(Check::Bullets, "a b", "- one\n- two"), None);
    assert_eq!(verdict(Check::Bullets, "a b", "One and two."), Some(Reason::NotBullets));
    assert_eq!(verdict(Check::Bullets, "a b", "- one\ntwo"), Some(Reason::NotBullets));
    assert_eq!(verdict(Check::Bullets, "a b", " \n"), Some(Reason::NotBullets));
}

/// An action-item list is dash lines, or the text itself when it holds no task; prose is refused.
#[test]
fn action_items_are_dash_lines_or_the_text_unchanged() {
    assert_eq!(verdict(Check::Items, "Nothing to do here.", "Nothing to do here."), None);
    assert_eq!(verdict(Check::Items, "Send it. Call Bo.", "- Send it\n- Call Bo"), None);
    assert_eq!(verdict(Check::Items, "Send it. Call Bo.", "You should send it and call Bo."), Some(Reason::NotBullets));
}

/// A shorter rewrite must not have more words, a summary no more characters than the text.
#[test]
fn shorter_and_summary_must_not_grow() {
    assert_eq!(verdict(Check::Shorter, "one two three four", "one two three"), None);
    assert_eq!(verdict(Check::Shorter, "one two three", "one two three four"), Some(Reason::NotShorter));
    assert_eq!(verdict(Check::Summary, "a long enough text here", "a text"), None);
    assert_eq!(verdict(Check::Summary, "a text", "a long enough text here"), Some(Reason::NotShorter));
}

/// A correction may fix a contraction but not flip a negation or drop a future, and may not add one.
#[test]
fn a_correction_keeps_negation_and_future() {
    assert_eq!(verdict(Check::Fix, "she dont like it", "She doesn't like it."), None);
    assert_eq!(verdict(Check::Fix, "she dont like it", "She does like it."), Some(Reason::LostNegation));
    assert_eq!(verdict(Check::Fix, "I will send it", "I sent it."), Some(Reason::LostNegation));
    assert_eq!(verdict(Check::Fix, "I send it tomorow", "I will send it tomorrow."), Some(Reason::LostNegation));
    assert_eq!(verdict(Check::Fix, "we cant go", "We can't go."), None);
    assert_eq!(verdict(Check::None, "x", "anything at all"), None);
}

/// Every refusal has a user-facing explanation and a distinct log code, so a failed check always says why.
#[test]
fn every_reason_has_an_explanation_and_a_code() {
    let all = [
        Reason::Empty,
        Reason::Refused,
        Reason::Runaway,
        Reason::LostProtected,
        Reason::OverEdited,
        Reason::NotBullets,
        Reason::NotShorter,
        Reason::LostNegation,
        Reason::MaskBroken,
    ];
    let codes: std::collections::HashSet<_> = all.iter().map(|r| r.code()).collect();
    assert_eq!(codes.len(), all.len());
    assert!(all.iter().all(|r| !r.explain().is_empty() && !r.code().contains(' ')));
}

/// The real tasks wire their check: the bullet task refuses prose, the summary task refuses a longer text, the correction task refuses a flipped negation.
#[test]
fn each_real_task_applies_its_own_check() {
    let judge = |action: Action, input: &str, answer: &str| accept_for(task(action).expect("model task"), input, answer);
    assert_eq!(judge(Action::BulletList, "Milk and eggs and bread.", "Milk, eggs and bread."), Verdict::Reject(Reason::NotBullets));
    assert!(matches!(judge(Action::BulletList, "Milk and eggs.", "- Milk\n- Eggs"), Verdict::Accept(_)));
    assert_eq!(
        judge(Action::Summarize, "Short text.", "A much longer text than the one it summarises."),
        Verdict::Reject(Reason::NotShorter)
    );
    assert_eq!(judge(Action::Correct, "we dont know", "We do know."), Verdict::Reject(Reason::LostNegation));
}

/// A reply or summary may omit a date from the incoming text; a correction may not.
#[test]
fn only_editing_tasks_must_keep_every_number() {
    let input = "Can we meet on 12/03 at 5pm?";
    assert!(matches!(accept_for(task(Action::WriteReply).expect("model task"), input, "Sure, I will be there."), Verdict::Accept(_)));
    assert_eq!(
        accept_for(task(Action::Correct).expect("model task"), input, "Can we meet on Tuesday?"),
        Verdict::Reject(Reason::LostProtected)
    );
}

/// The span finder covers Windows and Unix paths, UNC shares, bare commands (and not the prose after them) and amounts.
#[test]
fn spans_cover_paths_commands_and_amounts() {
    assert_eq!(spans(r"open C:\Users\me\notes.txt now"), [r"C:\Users\me\notes.txt"]);
    assert_eq!(spans(r"copy \\server\share\a.doc please"), [r"\\server\share\a.doc"]);
    assert_eq!(spans("edit ~/dev/cue/src/lib.rs today"), ["~/dev/cue/src/lib.rs"]);
    assert_eq!(spans("then run cargo test and relax"), ["cargo test"]);
    assert_eq!(spans("use git push --force origin/main here"), ["git push --force origin/main"]);
    assert_eq!(spans("it costs $5.50 or \u{20ac}1,200 total"), ["$5.50", "\u{20ac}1,200"]);
    assert_eq!(spans("see https://a.example/x?id=2 and bob@acme.com"), ["https://a.example/x?id=2", "bob@acme.com"]);
    assert!(spans("Nothing special in this sentence, in 2024.").is_empty());
}

/// Protected tokens add mentions, tags and numbers to the maskable spans.
#[test]
fn protected_adds_mentions_tags_and_numbers() {
    let found: Vec<&str> = PROTECTED.find_iter("ping @ana about #launch on 12/03, $5").map(|m| m.as_str()).collect();
    assert_eq!(found, ["@ana", "#launch", "12/03", "$5"]);
}

/// Lifted spans come back in place when the answer carries every placeholder once, in any order, and the text sent holds none of them.
#[test]
fn lifted_spans_are_restored_exactly_once() {
    let masked = mask::lift(r"see C:\a\b.txt and https://x.example/p then pay $5");
    assert!(!masked.text().contains("example") && !masked.text().contains('$') && !masked.text().contains("a\\b"));
    let swapped = format!("then pay {} after {} and {}", "\u{27e6}3\u{27e7}", "\u{27e6}2\u{27e7}", "\u{27e6}1\u{27e7}");
    assert_eq!(masked.restore(&swapped).as_deref(), Some(r"then pay $5 after https://x.example/p and C:\a\b.txt"));
}

/// An answer that drops, repeats or invents a placeholder is refused, so a protected span is never lost, doubled or forged.
#[test]
fn a_broken_placeholder_set_is_refused() {
    let masked = mask::lift("go to https://x.example and C:\\a\\b.txt");
    assert!(masked.restore("go to \u{27e6}1\u{27e7}").is_none());
    assert!(masked.restore("\u{27e6}1\u{27e7} \u{27e6}1\u{27e7} \u{27e6}2\u{27e7}").is_none());
    assert!(masked.restore("\u{27e6}1\u{27e7} \u{27e6}2\u{27e7} \u{27e6}9\u{27e7}").is_none());
    assert!(masked.restore("\u{27e6}1\u{27e7} \u{27e6}2\u{27e7}").is_some());
}

/// When the text already uses the first placeholder mark, another style is chosen; a text with nothing to lift is sent as it is.
#[test]
fn the_placeholder_style_never_collides_with_the_text() {
    let masked = mask::lift("a \u{27e6}1\u{27e7} b https://x.example");
    assert!(masked.text().contains("\u{ab}1\u{bb}") && masked.text().contains("\u{27e6}1\u{27e7}"));
    assert_eq!(masked.restore("a \u{27e6}1\u{27e7} b \u{ab}1\u{bb}").as_deref(), Some("a \u{27e6}1\u{27e7} b https://x.example"));
    let plain = mask::lift("nothing to lift");
    assert_eq!((plain.text(), plain.restore("changed").as_deref()), ("nothing to lift", Some("changed")));
}

/// A masked task never shows the model a path, and the answer gets the path back; the model touching a placeholder is a refusal that keeps the text.
#[test]
fn the_pipeline_hides_spans_from_the_model_and_restores_them() {
    let backend = scripted(|sent| Ok(sent.replace("Open", "Please open")));
    let outcome = Pipeline::new(&backend, &Cancel::never()).run(&MASKED, r"Open C:\Users\me\notes.txt now.");
    assert!(!backend.seen.borrow()[0].contains("Users"));
    assert_eq!(outcome, Outcome::Edited { text: r"Please open C:\Users\me\notes.txt now.".into(), notice: None });
    let broken = scripted(|_| Ok("Please open the file.".into()));
    let outcome = Pipeline::new(&broken, &Cancel::never()).run(&MASKED, r"Open C:\Users\me\notes.txt now.");
    assert_eq!(outcome, Outcome::NoUsableAnswer(Reason::MaskBroken));
}
