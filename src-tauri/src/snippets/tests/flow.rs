//! File: snippets/tests/flow.rs
//! Purpose: Protect what a key starts and how an expansion ends: late keys are replayed in order inside the call that ends the expansion, so the expander is never visibly idle between them, and a failed expansion or undo never replays them.
//! Contents: scenario tests of `Flow` with the real `Matcher`, `Tracker` and `Phase`.

use super::helpers::effective;
use crate::platform::Typed::{self, Backspace, Char};
use crate::snippets::expand::flow::{Flow, Next};
use crate::snippets::expand::matcher::Matcher;
use crate::snippets::expand::phase::Phase;

/// Types `text` into the flow. Inputs: the flow, the triggers and the text. Returns what each key started, in order.
fn type_into(flow: &mut Flow<&'static str>, matcher: &Matcher, text: &str) -> Vec<Next<&'static str>> {
    text.chars().map(|c| flow.key(Char(c), matcher)).collect()
}

/// The keys as typed characters.
fn chars(text: &str) -> Vec<Typed> {
    text.chars().map(Char).collect()
}

/// A flow in the middle of an expansion that has been sealed, with `late` typed after the seal.
fn committing(matcher: &Matcher, late: &str) -> Flow<&'static str> {
    let mut flow = Flow::default();
    assert!(matches!(type_into(&mut flow, matcher, ":sig").pop(), Some(Next::Expand(_))));
    assert_eq!(flow.phase.seal().as_deref(), Some(""));
    type_into(&mut flow, matcher, late);
    flow
}

/// Late keys that complete a trigger start its expansion from the same call that ends the first one, and the keys after it are held for it in order.
#[test]
fn ending_replays_late_keys_in_one_call_and_holds_the_rest() {
    let matcher = Matcher::new(&[effective(1, ":sig"), effective(2, ":a")]);
    let mut flow = committing(&matcher, ":ax");
    let (next, withheld) = flow.end(true, &matcher);
    assert_eq!(withheld, 0);
    assert!(matches!(next, Next::Expand(hit) if hit.id == 2));
    assert_eq!(flow.phase, Phase::Preparing(chars("x")));
    assert_eq!(flow.key(Char('y'), &matcher), Next::Held);
    assert_eq!(flow.phase, Phase::Preparing(chars("xy")));
}

/// Late keys that complete nothing leave the expander idle and tracked, so the rest of a trigger typed afterwards still fires.
#[test]
fn ending_without_a_trigger_returns_to_idle() {
    let matcher = Matcher::new(&[effective(1, ":sig")]);
    let mut flow = committing(&matcher, ":s");
    assert_eq!(flow.end(true, &matcher), (Next::Nothing, 0));
    assert_eq!(flow.phase, Phase::Idle);
    assert!(matches!(type_into(&mut flow, &matcher, "ig").pop(), Some(Next::Expand(_))));
}

/// After a failed delivery the late keys, even a whole trigger, are counted and never replayed.
#[test]
fn a_failed_expansion_withholds_a_late_trigger() {
    let matcher = Matcher::new(&[effective(1, ":sig")]);
    let mut flow = committing(&matcher, ":sig");
    assert_eq!(flow.end(false, &matcher), (Next::Nothing, 4));
    assert_eq!(flow.phase, Phase::Idle);
}

/// A failed undo (the same `false` the undo worker returns) withholds the keys typed meanwhile, so no destructive expansion starts on uncertain text.
#[test]
fn a_failed_undo_withholds_the_keys_typed_meanwhile() {
    let matcher = Matcher::new(&[effective(1, ":sig")]);
    let mut flow = Flow { undo: Some("restore"), ..Flow::default() };
    assert_eq!(flow.key(Backspace, &matcher), Next::Undo("restore"));
    type_into(&mut flow, &matcher, ":sig");
    assert_eq!(flow.end(false, &matcher), (Next::Nothing, 4));
    assert_eq!(flow.phase, Phase::Idle);
}
