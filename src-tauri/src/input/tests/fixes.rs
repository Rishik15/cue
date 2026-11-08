//! File: input/tests/fixes.rs
//! Purpose: Protect the review round's findings through the scripted keyboard: a release that could not be queued, rebased writes, repeats, observers under a surface, popup stall, surface failure, deadline moves, late failures, recovery, request cancel before the open answers, and the partial restore of a lift.
//! Contents: one scenario test per finding.

use std::cell::RefCell;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::{drain, fake, terminals, window};
use crate::input::bridge::Held;
use crate::input::fake::{Fake, FakeStore};
use crate::input::lift::{plan, reconcile, KeyBatch};
use crate::input::{InputError, Key, Mods, Rect, Verdict};

const PASS: Verdict = Verdict::Pass;
const SWALLOW: Verdict = Verdict::Swallow;

/// A Hold shortcut whose release could not be queued is still released: the debt is recorded and paid when the effects side looks.
#[test]
fn a_refused_release_is_still_delivered() {
    let f = fake();
    f.mod_down(Mods::CTRL_L);
    assert_eq!(f.down(Key::Space), SWALLOW);
    assert_eq!(f.active(), vec![0]);
    f.refuse(true);
    f.up(Key::Space);
    f.refuse(false);
    assert!(f.active().is_empty(), "the hold must not stay started");
    assert_eq!(f.log.lock().unwrap().actions.iter().filter(|a| **a == (0, false)).count(), 1, "released once");
}

/// A write behind a failing one is not written when the failure makes it invalid: only one write is with the writer at a time.
#[test]
fn a_rebased_away_job_never_writes() {
    let store = FakeStore::new();
    store.put("grab", serde_json::json!({"modifiers": "CTRL_LEFT", "key": "K"}));
    let f = Fake::with_store(&super::CAT, store.clone());
    store.block();
    let owner = f.input.owner(window()).unwrap();
    let _a = owner.record("grab").unwrap();
    f.mod_down(Mods::CTRL_L);
    f.down(Key::L);
    f.up(Key::L);
    let _b = owner.record("selection").unwrap();
    f.down(Key::K);
    f.up(Key::K);
    store.fail.store(true, Ordering::Release);
    store.unblock();
    f.settle(1);
    std::thread::sleep(Duration::from_millis(100));
    f.pump();
    assert_eq!(store.writes_len(), 1, "the second write was dropped by the rebase before it reached the writer");
    assert!(store.get("selection").is_none());
}

/// No thread is started inside the hook: a finished combination waits for the message loop to hand it to the writer.
#[test]
fn the_writer_is_not_started_inside_the_callback() {
    let f = fake();
    f.auto_pump.store(false, Ordering::Release);
    let owner = f.input.owner(window()).unwrap();
    let _rec = owner.record("grab").unwrap();
    f.mod_down(Mods::CTRL_L);
    f.down(Key::K);
    f.up(Key::K);
    assert!(f.writes_due());
    assert_eq!(f.store.writes_len(), 0);
    f.drive();
    f.settle(1);
    assert!(!f.writes_due());
}

/// A cancel for a request that has not been opened yet, or whose open is still on its way, makes that open do nothing.
#[test]
fn cancel_before_the_open_answers_wins() {
    let f = fake();
    let mut held = Held::new(f.input.owner(window()).unwrap());
    held.cancel(5);
    assert!(held.open("grab", 5).is_err());
    let view = held.open("grab", 6).unwrap();
    assert!(view.open);
    assert!(held.get(6).is_some());
    held.cancel(6);
    assert!(held.get(6).is_none(), "the handle is freed");
    f.pump();
    f.mod_down(Mods::CTRL_L);
    assert_eq!(f.down(Key::K), PASS, "a cancelled recording captures nothing");
}

/// A repeat of a key whose press went to the app never starts a shortcut, and neither does a repeat after the modifier came back.
#[test]
fn repeats_do_not_start_actions() {
    let f = fake();
    assert_eq!(f.down(Key::Space), PASS);
    f.mod_down(Mods::CTRL_L);
    assert_eq!(f.down(Key::Space), PASS, "the repeat belongs to the first press");
    assert!(f.active().is_empty());
    f.up(Key::Space);
    f.mod_up(Mods::CTRL_L);

    f.mod_down(Mods::CTRL_L);
    assert_eq!(f.down(Key::Space), SWALLOW);
    f.mod_up(Mods::CTRL_L);
    assert!(f.active().is_empty());
    f.mod_down(Mods::CTRL_L);
    assert_eq!(f.down(Key::Space), SWALLOW, "still the first press, so it stays captured");
    assert!(f.active().is_empty(), "a repeat is not a fresh press");
}

/// Escape and typing still reach the observers while a recording holds the keyboard.
#[test]
fn observers_hear_keys_under_a_recording() {
    let f = fake();
    let owner = f.input.owner(window()).unwrap();
    let _rec = owner.record("grab").unwrap();
    f.down(Key::A);
    f.down(Key::Escape);
    let log = f.log.lock().unwrap();
    assert_eq!(log.escapes, 1);
    assert!(log.typing >= 1);
}

/// A popup that never answers gives its keys back after the stall limit.
#[test]
fn a_stalled_popup_stops_owning_keys() {
    let f = fake();
    let owner = f.input.owner(window()).unwrap();
    let told = Arc::new(Mutex::new(0u32));
    let seen = told.clone();
    let _lease = owner
        .tray_popup(Rect { left: 0, top: 0, right: 10, bottom: 10 }, move |_| {
            *seen.lock().unwrap() += 1;
            true
        })
        .unwrap();
    f.pump();
    assert_eq!(f.down(Key::Escape), SWALLOW);
    f.up(Key::Escape);
    assert!(f.next_deadline().is_some());
    f.advance(Duration::from_secs(31));
    assert_eq!(f.down(Key::Escape), PASS);
    assert!(f.next_deadline().is_none());
}

/// A part a recording needs that could not be made ends it with a reason and leaves nothing captured.
#[test]
fn failed_auxiliary_setup_ends_the_recording() {
    let f = fake();
    let owner = f.input.owner(window()).unwrap();
    let rec = owner.record("grab").unwrap();
    let sub = rec.observe();
    f.fail_surfaces("no mouse hook");
    assert_eq!(terminals(&drain(&sub)), 1);
    assert_eq!(f.down(Key::A), PASS);
}

/// An idle deadline that moved is the one the timer must follow.
#[test]
fn a_keypress_moves_the_deadline() {
    let f = fake();
    let owner = f.input.owner(window()).unwrap();
    let _rec = owner.record("grab").unwrap();
    let first = f.next_deadline().unwrap();
    f.advance(Duration::from_secs(59));
    f.down(Key::A);
    f.up(Key::A);
    let moved = f.next_deadline().unwrap();
    assert!(moved >= first + Duration::from_secs(59), "the 60 second limit starts again from the key");
}

/// A save that fails after its recording ended is reported to the user, not only kept in the catalog.
#[test]
fn a_late_failure_is_reported() {
    let f = fake();
    f.store.block();
    f.store.fail.store(true, Ordering::Release);
    let owner = f.input.owner(window()).unwrap();
    let rec = owner.record("grab").unwrap();
    f.mod_down(Mods::CTRL_L);
    f.down(Key::K);
    f.up(Key::K);
    let _ = rec.cancel();
    f.pump();
    f.store.unblock();
    f.settle(1);
    let log = f.log.lock().unwrap();
    assert!(log.reports.iter().any(|m| m.contains("could not be saved")), "{:?}", log.reports);
}

/// An explicit recovery waits for the hook to be made again before it answers.
#[test]
fn recover_answers_after_the_hook_is_reinstalled() {
    let f = fake();
    f.hold_arm.store(true, Ordering::Release);
    let done = f.input.recover();
    f.pump();
    assert!(done.try_get().is_none(), "not answered while the hook is still being replaced");
    f.finish_recover();
    assert!(matches!(done.wait(Duration::from_secs(1)), Some(Ok(()))));
}

/// The activity mark and Mark are public, as the frozen interface says.
#[test]
fn the_activity_mark_is_public() {
    let mark: crate::input::Mark = crate::input::activity_mark();
    let _ = mark.physical_keys_since();
}

/// A restore that was only partly sent still releases what it did press once the user let go, and reports the failure.
#[test]
fn a_partial_restore_is_reconciled() {
    let sent_batches: RefCell<Vec<KeyBatch>> = RefCell::new(Vec::new());
    let both = Mods::CTRL_L | Mods::SHIFT_L;
    let (report, failure) = reconcile(both, &|| Mods::SHIFT_L, &mut |batch| {
        sent_batches.borrow_mut().push(batch.clone());
        if sent_batches.borrow().len() == 1 {
            (1, Some(InputError::InjectionPartial { sent: 1, wanted: 2 }))
        } else {
            (batch.0.len(), None)
        }
    });
    assert!(failure.is_some());
    let batches = sent_batches.borrow();
    assert!(batches.len() >= 2, "a second send releases the key that was pressed and then let go");
    assert_eq!(batches[1], plan(Mods::CTRL_L).0);
    assert_eq!(report.released, Mods::CTRL_L);
}
