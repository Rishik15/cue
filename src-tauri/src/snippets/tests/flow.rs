//! File: snippets/tests/flow.rs
//! Purpose: Protect what a key starts and how an expansion ends, through the one owner of that state: late keys are replayed in order inside the call that ends the expansion, so the expander is never visibly idle between them; a failed expansion or undo never replays them; and work pending when the expander is stopped or its triggers are swapped can no longer type, arm an undo or start anything.
//! Contents: scenario tests of `Flow` with the real `Matcher` and `Tracker`, of the use-count write ordered before a later `{counter}` read (`count`), of the completion policy, of a stop reaching a question and of the delivery's `proceed` hook (`may_send`: no send once stale or after a late key), and of the completion sound's volume.

use super::helpers::effective;
use crate::platform::InsertError;
use crate::platform::Target;
use crate::platform::Typed::{self, Backspace, Char};
use crate::snippets::ask;
use crate::snippets::expand::count;
use crate::snippets::expand::flow::{Flow, Next, Outcome};
use crate::snippets::expand::matcher::{Hit, Matcher};
use crate::snippets::expand::output::{complete, options, Hooks};
use crate::snippets::expand::template::Argument;
use crate::snippets::expand::worker::completion_volume;
use std::time::Duration;

type Test = Flow<&'static str>;

/// Types `text` into the flow. Inputs: the flow, the triggers and the text. Returns what each key started, in order.
fn type_into(flow: &mut Test, matcher: &Matcher, text: &str) -> Vec<Next<&'static str>> {
    text.chars().map(|c| flow.key(Char(c), matcher)).collect()
}

/// A flow whose expansion is under way (`:sig` was typed) and not sealed yet.
fn preparing(matcher: &Matcher) -> Test {
    let mut flow = Flow::default();
    assert!(matches!(type_into(&mut flow, matcher, ":sig").pop(), Some(Next::Expand(_))));
    flow
}

/// A flow in the middle of an expansion that has been sealed, with `late` typed after the seal.
fn committing(matcher: &Matcher, late: &str) -> Test {
    let mut flow = preparing(matcher);
    assert_eq!(flow.seal().as_deref(), Some(""));
    type_into(&mut flow, matcher, late);
    flow
}

/// Late keys that complete a trigger start its expansion from the same call that ends the first one, and the keys after it are held for it in order.
#[test]
fn ending_replays_late_keys_in_one_call_and_holds_the_rest() {
    let matcher = Matcher::new(&[effective(1, ":sig"), effective(2, ":a")]);
    let mut flow = committing(&matcher, ":ax");
    let (next, withheld) = flow.end(Outcome::Delivered(None), &matcher);
    assert_eq!(withheld, 0);
    assert!(matches!(next, Next::Expand(hit) if hit.id == 2));
    assert_eq!(flow.key(Char('y'), &matcher), Next::Held);
    assert_eq!(flow.seal().as_deref(), Some("xy"));
}

/// Late keys that complete nothing leave the expander idle and tracked, so the rest of a trigger typed afterwards still fires.
#[test]
fn ending_without_a_trigger_returns_to_idle() {
    let matcher = Matcher::new(&[effective(1, ":sig")]);
    let mut flow = committing(&matcher, ":s");
    assert_eq!(flow.end(Outcome::Delivered(None), &matcher), (Next::Nothing, 0));
    assert_eq!(flow.seal(), None, "nothing is running");
    assert!(matches!(type_into(&mut flow, &matcher, "ig").pop(), Some(Next::Expand(_))));
}

/// After a failed delivery the late keys, even a whole trigger, are counted and never replayed.
#[test]
fn a_failed_expansion_withholds_a_late_trigger() {
    let matcher = Matcher::new(&[effective(1, ":sig")]);
    let mut flow = committing(&matcher, ":sig");
    assert_eq!(flow.end(Outcome::Failed, &matcher), (Next::Nothing, 4));
    assert_eq!(flow.seal(), None);
}

/// A flow whose delivered expansion armed an undo, so Backspace takes it back.
fn armed(matcher: &Matcher) -> Test {
    let mut flow = preparing(matcher);
    assert_eq!(flow.seal().as_deref(), Some(""));
    assert_eq!(flow.end(Outcome::Delivered(Some("restore")), matcher), (Next::Nothing, 0));
    flow
}

/// A failed undo (the same failure the undo worker returns) withholds the keys typed meanwhile, so no destructive expansion starts on uncertain text.
#[test]
fn a_failed_undo_withholds_the_keys_typed_meanwhile() {
    let matcher = Matcher::new(&[effective(1, ":sig")]);
    let mut flow = armed(&matcher);
    assert_eq!(flow.key(Backspace, &matcher), Next::Undo("restore"));
    type_into(&mut flow, &matcher, ":sig");
    assert_eq!(flow.end(Outcome::Failed, &matcher), (Next::Nothing, 4));
}

/// Keys typed while an expansion prepares come back in order as the text typed meanwhile, with Backspace removing one.
#[test]
fn sealing_returns_the_keys_typed_meanwhile_in_order() {
    let matcher = Matcher::new(&[effective(1, ":sig")]);
    let mut flow = preparing(&matcher);
    type_into(&mut flow, &matcher, "hi");
    assert_eq!(flow.seal().as_deref(), Some("hi"));
    let mut flow = preparing(&matcher);
    type_into(&mut flow, &matcher, "ab");
    assert_eq!(flow.key(Backspace, &matcher), Next::Held);
    assert_eq!(flow.seal().as_deref(), Some("a"));
}

/// Backspace into the trigger, Enter and a caret move all stop the expansion, yet later keys are still kept and handed back.
#[test]
fn abort_keys_stop_the_expansion_and_later_keys_are_still_kept() {
    let matcher = Matcher::new(&[effective(1, ":sig")]);
    for keys in [vec![Backspace], vec![Char('x'), Char('\n')], vec![Typed::Reset]] {
        let mut flow = preparing(&matcher);
        keys.into_iter().for_each(|k| assert_eq!(flow.key(k, &matcher), Next::Held));
        assert_eq!(flow.seal(), None);
        assert_eq!(flow.key(Char('z'), &matcher), Next::Held);
        assert_eq!(flow.end(Outcome::Failed, &matcher), (Next::Nothing, 1));
    }
}

/// A trigger completed while an expansion runs is held for it, not started, and sealing twice does not take the late keys.
#[test]
fn a_busy_flow_holds_keys_and_sealing_twice_leaves_late_keys_alone() {
    let matcher = Matcher::new(&[effective(1, ":sig")]);
    let mut flow = committing(&matcher, ":sig");
    assert_eq!(flow.seal(), None);
    assert_eq!(flow.end(Outcome::Failed, &matcher), (Next::Nothing, 4));
    let mut idle = Test::default();
    assert_eq!(idle.end(Outcome::Delivered(None), &matcher), (Next::Nothing, 0), "an idle flow has nothing to give back");
}

/// Stopping the expander while an expansion waits (an `{argument}` dialog): the worker's seal refuses, ending replays nothing and starts nothing even for a whole trigger typed meanwhile, and keys typed afterwards start nothing.
#[test]
fn stopping_invalidates_the_pending_expansion() {
    let matcher = Matcher::new(&[effective(1, ":sig")]);
    let mut flow = preparing(&matcher);
    type_into(&mut flow, &matcher, ":sig");
    flow.stop();
    assert_eq!(flow.seal(), None);
    assert_eq!(flow.end(Outcome::Delivered(Some("restore")), &matcher), (Next::Nothing, 0));
    assert_eq!(flow.key(Backspace, &matcher), Next::Nothing, "no undo was armed");
    assert!(type_into(&mut flow, &matcher, ":sig").iter().all(|next| *next == Next::Nothing));
}

/// Swapping the triggers drops an armed undo.
#[test]
fn replacing_the_index_drops_an_armed_undo() {
    let matcher = Matcher::new(&[effective(1, ":sig")]);
    let mut flow = armed(&matcher);
    flow.replace_index();
    assert_eq!(flow.key(Backspace, &matcher), Next::Nothing);
}

/// Swapping the triggers while an expansion waits: it may not type (seal refuses) and arms no undo, but the keys typed meanwhile are replayed through the new triggers.
#[test]
fn replacing_the_index_invalidates_the_pending_expansion_but_keeps_its_keys() {
    let old = Matcher::new(&[effective(1, ":sig")]);
    let new = Matcher::new(&[effective(2, ":a")]);
    let mut flow = preparing(&old);
    flow.replace_index();
    assert_eq!(flow.seal(), None);
    assert_eq!(flow.end(Outcome::Delivered(Some("restore")), &new), (Next::Nothing, 0));
    assert_eq!(flow.key(Backspace, &new), Next::Nothing, "the stale expansion armed nothing");

    let mut flow = preparing(&old);
    type_into(&mut flow, &old, ":a");
    flow.replace_index();
    assert!(matches!(flow.end(Outcome::Delivered(None), &new), (Next::Expand(hit), 0) if hit.id == 2));
}

/// The completion sound follows the Notes alert volume as it is now: off gives silence, on gives the volume capped at 100.
#[test]
fn the_completion_sound_volume_follows_the_setting() {
    assert_eq!(completion_volume(false, 70), None);
    assert_eq!(completion_volume(true, 0), Some(0));
    assert_eq!(completion_volume(true, 70), Some(70));
    assert_eq!(completion_volume(true, 500), Some(100));
}

/// During an injection delay a key that completes a longer trigger (`@@p` after `@@`) replaces the waiting expansion, which erases all three characters; a key that completes nothing leaves it to go ahead with that key as text typed meanwhile.
#[test]
fn a_longer_trigger_typed_in_the_delay_replaces_the_waiting_one() {
    let matcher = Matcher::new(&[effective(1, "@@"), effective(2, "@@p")]);
    let mut flow: Test = Flow::default();
    let Some(Next::Expand(first)) = type_into(&mut flow, &matcher, "@@").pop() else { panic!("no hit") };
    assert_eq!(first.id, 1);
    assert_eq!(flow.key(Char('p'), &matcher), Next::Held);
    match flow.extend(&matcher, &first) {
        Some(Next::Expand(hit)) => assert_eq!((hit.id, hit.erase), (2, 3)),
        other => panic!("{other:?}"),
    }
    assert_eq!(flow.seal().as_deref(), Some(""));

    let mut flow: Test = Flow::default();
    let Some(Next::Expand(first)) = type_into(&mut flow, &matcher, "@@").pop() else { panic!("no hit") };
    flow.key(Char('x'), &matcher);
    assert_eq!(flow.extend(&matcher, &first), None);
    assert_eq!(flow.seal().as_deref(), Some("x"));
}

/// A waiting trigger plus the flow after it fired. Inputs: the triggers and the text typed to complete the first one. Returns the flow and the waiting hit.
fn waiting(matcher: &Matcher, text: &str) -> (Test, Hit) {
    let mut flow: Test = Flow::default();
    let Some(Next::Expand(first)) = type_into(&mut flow, matcher, text).pop() else { panic!("no hit") };
    (flow, first)
}

/// An unrelated trigger typed during the delay (`:a` after `@@`) does not replace the waiting one: `@@` still expands and the typed keys stay behind it as text.
#[test]
fn an_unrelated_trigger_in_the_delay_keeps_the_waiting_expansion() {
    let matcher = Matcher::new(&[effective(1, "@@"), effective(2, ":a")]);
    let (mut flow, first) = waiting(&matcher, "@@");
    type_into(&mut flow, &matcher, ":a");
    assert_eq!(flow.extend(&matcher, &first), None);
    assert_eq!(flow.seal().as_deref(), Some(":a"));
}

/// A caret move typed during the delay still cancels the delivery, even when a trigger follows it.
#[test]
fn a_reset_in_the_delay_still_cancels_when_a_trigger_follows() {
    let matcher = Matcher::new(&[effective(1, "@@"), effective(2, ":a")]);
    let (mut flow, first) = waiting(&matcher, "@@");
    flow.key(Typed::Reset, &matcher);
    type_into(&mut flow, &matcher, ":a");
    assert_eq!(flow.extend(&matcher, &first), None);
    assert_eq!(flow.seal(), None);
}

/// A caret move, then the whole longer trigger retyped, is not an extension of the waiting one either.
#[test]
fn a_reset_then_the_longer_trigger_retyped_cancels() {
    let matcher = Matcher::new(&[effective(1, "@@"), effective(2, "@@p")]);
    let (mut flow, first) = waiting(&matcher, "@@");
    flow.key(Typed::Reset, &matcher);
    type_into(&mut flow, &matcher, "@@p");
    assert_eq!(flow.extend(&matcher, &first), None);
    assert_eq!(flow.seal(), None);
}

/// A longer trigger that starts with a space still wins when its last letter arrives in the delay.
#[test]
fn a_leading_space_longer_trigger_typed_in_the_delay_wins() {
    let matcher = Matcher::new(&[effective(1, " @@"), effective(2, " @@p")]);
    let (mut flow, first) = waiting(&matcher, " @@");
    assert_eq!(flow.key(Char('p'), &matcher), Next::Held);
    match flow.extend(&matcher, &first) {
        Some(Next::Expand(hit)) => assert_eq!((hit.id, hit.erase), (2, 3)),
        other => panic!("{other:?}"),
    }
}

/// The waiting trigger typed again after other keys (`x@@` after `@@`) is a separate trigger, not an extension: the waiting expansion goes ahead and the keys stay its text typed meanwhile.
#[test]
fn a_repeated_trigger_in_the_delay_is_not_an_extension() {
    let matcher = Matcher::new(&[effective(1, "@@")]);
    let (mut flow, first) = waiting(&matcher, "@@");
    type_into(&mut flow, &matcher, "x@@");
    assert_eq!(flow.extend(&matcher, &first), None);
    assert_eq!(flow.seal().as_deref(), Some("x@@"));
}

/// A longer trigger that begins with the waiting one but is typed after other keys (`@@p` after `@@x`) does not span the waiting trigger, so it is not an extension either; `@@p` typed directly still wins.
#[test]
fn a_separated_longer_trigger_in_the_delay_is_not_an_extension() {
    let matcher = Matcher::new(&[effective(1, "@@"), effective(2, "@@p")]);
    let (mut flow, first) = waiting(&matcher, "@@");
    type_into(&mut flow, &matcher, "x@@p");
    assert_eq!(flow.extend(&matcher, &first), None);
    assert_eq!(flow.seal().as_deref(), Some("x@@p"));
}

/// A dispatched undo is still current when nothing intervenes, and ends like any worker (nothing re-armed).
#[test]
fn a_dispatched_undo_is_current_when_nothing_intervenes() {
    let matcher = Matcher::new(&[effective(1, ":sig")]);
    let mut flow = armed(&matcher);
    assert_eq!(flow.key(Backspace, &matcher), Next::Undo("restore"));
    assert!(flow.current());
    assert_eq!(flow.end(Outcome::Delivered(None), &matcher), (Next::Nothing, 0));
    assert_eq!(flow.key(Backspace, &matcher), Next::Nothing);
}

/// A stop after Backspace dispatched the undo makes it stale before it erases anything, and its end starts nothing.
#[test]
fn a_stop_after_dispatch_makes_the_undo_stale() {
    let matcher = Matcher::new(&[effective(1, ":sig")]);
    let mut flow = armed(&matcher);
    assert_eq!(flow.key(Backspace, &matcher), Next::Undo("restore"));
    flow.stop();
    assert!(!flow.current());
    assert_eq!(flow.end(Outcome::Delivered(None), &matcher), (Next::Nothing, 0));
}

/// Swapping the triggers after Backspace dispatched the undo makes it stale too.
#[test]
fn an_index_swap_after_dispatch_makes_the_undo_stale() {
    let matcher = Matcher::new(&[effective(1, ":sig")]);
    let mut flow = armed(&matcher);
    assert_eq!(flow.key(Backspace, &matcher), Next::Undo("restore"));
    flow.replace_index();
    assert!(!flow.current());
    assert_eq!(flow.end(Outcome::Delivered(None), &matcher), (Next::Nothing, 0));
}

/// The completion policy on real `put` results: a warning counts the use, reports and arms no undo; a clean put counts and arms; a failure counts nothing and fails the expansion (so no replay), whether it is certain or uncertain.
#[test]
fn a_put_warning_counts_as_delivered_but_a_failure_does_not() {
    let clean = complete(Ok(()), || Some("undo"));
    assert_eq!((clean.outcome, clean.counted, clean.report), (Outcome::Delivered(Some("undo")), true, None));
    for warning in [InsertError::NotRestored, InsertError::KeepCopy, InsertError::Submit("x".into()), InsertError::Caret("x".into())] {
        let done = complete(Err(warning), || panic!("a warned delivery arms no undo"));
        assert_eq!((done.outcome, done.counted, done.report.is_some()), (Outcome::<&str>::Delivered(None), true, true));
    }
    for failure in
        [InsertError::Elevated, InsertError::FocusMoved { copied: true }, InsertError::Uncertain { why: "3/12".into(), copied: false }]
    {
        let done = complete(Err(failure), || panic!("a failed delivery arms no undo"));
        assert_eq!((done.outcome, done.counted, done.report.is_some()), (Outcome::<&str>::Failed, false, true));
    }
}

/// A question is refused when the expander stopped before it was registered, and one already open is cancelled by `close` (the waiting worker gets a cancel); the cases share the one static slot, so they run in one test. The slot stays taken until the cancelled worker releases it, so a restarted expander's question is refused meanwhile and registers after, and the old worker's release never touches a newer question.
#[test]
fn a_stop_cancels_a_question_open_or_about_to_open() {
    let args = || vec![Argument { label: "who".into(), choices: vec![] }];
    assert!(ask::claim(&args(), &|| false).is_none());
    assert!(!ask::is_open());
    let old = ask::claim(&args(), &|| true).expect("registered");
    ask::close();
    assert_eq!(old.recv().unwrap(), None);
    assert!(ask::claim(&args(), &|| true).is_none(), "the old worker still owns the slot");
    ask::release();
    assert!(!ask::is_open());
    let new = ask::claim(&args(), &|| true).expect("registered after the old worker is done");
    assert!(new.try_recv().is_err(), "the old cancel did not reach the new question");
    ask::release();
}

/// The options `put` builds carry the caller's `proceed` hook (a cancelled delivery types nothing) and `on_sent`; with no hook a delivery is never cancelled, and a hook built on `may_send` cancels it after a key typed since the seal.
#[test]
fn the_delivery_options_carry_the_proceed_hook() {
    let sent = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let flag = sent.clone();
    let hooks =
        Hooks { on_sent: Some(Box::new(move || flag.store(true, std::sync::atomic::Ordering::SeqCst))), proceed: Some(Box::new(|| false)) };
    let built = options(&Target::default(), Duration::ZERO, hooks);
    assert!(!built.may_proceed());
    (built.on_sent.as_ref().expect("on_sent carried"))();
    assert!(sent.load(std::sync::atomic::Ordering::SeqCst));
    assert!(options(&Target::default(), Duration::ZERO, Hooks::default()).may_proceed());
    let late = std::sync::Mutex::new(committing(&Matcher::new(&[effective(1, ":sig")]), "x"));
    let hooks = Hooks { proceed: Some(Box::new(move || late.lock().unwrap().may_send())), ..Default::default() };
    assert!(!options(&Target::default(), Duration::ZERO, hooks).may_proceed(), "a key after the seal cancels the delivery");
}

/// A seal with no key after it may send.
#[test]
fn may_send_is_true_after_a_quiet_seal() {
    let matcher = Matcher::new(&[effective(1, ":sig")]);
    let mut flow = preparing(&matcher);
    assert!(flow.may_send());
    assert_eq!(flow.seal().as_deref(), Some(""));
    assert!(flow.may_send());
}

/// A key typed after the seal is already in the app, so the batch fixed at the seal may not go out; the aborted delivery ends as delivered with nothing typed, the key is replayed as ordinary typing and the flow is idle again.
#[test]
fn a_key_after_the_seal_stops_the_send_and_is_replayed() {
    let matcher = Matcher::new(&[effective(1, ":sig")]);
    let mut flow = committing(&matcher, "x");
    assert!(!flow.may_send());
    assert_eq!(flow.end(Outcome::Delivered(None), &matcher), (Next::Nothing, 0));
    assert!(matches!(type_into(&mut flow, &matcher, ":sig").pop(), Some(Next::Expand(_))), "idle: a following trigger expands again");
}

/// A stopped expander or swapped triggers make the running expansion stale, so it may not send even with no late key.
#[test]
fn a_stopped_or_swapped_flow_may_not_send() {
    let matcher = Matcher::new(&[effective(1, ":sig")]);
    let mut flow = committing(&matcher, "");
    flow.stop();
    assert!(!flow.may_send());
    let mut flow = committing(&matcher, "");
    flow.replace_index();
    assert!(!flow.may_send());
}

/// A `{counter}` read that starts after the use-count write was claimed (the expansion had not ended yet) sees the written count, however the threads are scheduled.
#[test]
fn a_counter_read_after_the_claim_sees_the_written_count() {
    use std::sync::atomic::{AtomicI64, Ordering};
    use std::sync::Arc;
    let uses = Arc::new(AtomicI64::new(4));
    let claim = count::claim();
    let reader = std::thread::spawn({
        let uses = uses.clone();
        move || count::read(|| uses.load(Ordering::SeqCst) + 1)
    });
    claim.write(|| uses.store(5, Ordering::SeqCst));
    assert_eq!(reader.join().expect("reader"), 6);
}
