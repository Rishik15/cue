//! File: input/tests/recording.rs
//! Purpose: Protect shortcut recording through the public interface: immediate Escape, a recorded chord, cancel before Armed, keys held before and during, outside clicks, stale and replaced recordings, the tray popup taking over, deadlines, and exactly one final event.
//! Contents: scenario tests of `Owner::record`, `Recording::observe` and `cancel`, `Owner::tray_popup`.

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::{drain, fake, terminals, window};
use crate::input::commands::Command;
use crate::input::core::Verdict;
use crate::input::fake::Fake;
use crate::input::view::Hub;
use crate::input::{Event, Key, Mods, Phase, PopupKey, Reason};

const PASS: Verdict = Verdict::Pass;
const SWALLOW: Verdict = Verdict::Swallow;

fn last_event(views: &[crate::input::RecorderView]) -> Event {
    views.last().map(|v| v.event.clone()).unwrap()
}

/// Record then Escape at once: the recorder is armed, Escape cancels it, and exactly one final event arrives.
#[test]
fn immediate_escape_cancels_once() {
    let f = fake();
    let owner = f.input.owner(window()).unwrap();
    let rec = owner.record("grab").unwrap();
    let sub = rec.observe();
    assert_eq!(f.down(Key::Escape), SWALLOW);
    assert_eq!(f.up(Key::Escape), SWALLOW);
    let views = drain(&sub);
    assert_eq!(terminals(&views), 1);
    assert_eq!(last_event(&views), Event::Cancelled { reason: Reason::User });
    assert_eq!(f.down(Key::Escape), PASS);
}

/// A chord is captured whole, saved in order, and becomes the live binding only after the write; the recording stays open.
#[test]
fn a_recorded_chord_is_saved_and_activates() {
    let f = fake();
    let owner = f.input.owner(window()).unwrap();
    let rec = owner.record("grab").unwrap();
    let sub = rec.observe();
    assert_eq!(f.mod_down(Mods::CTRL_L), SWALLOW);
    assert_eq!(f.down(Key::K), SWALLOW);
    assert_eq!(f.up(Key::K), SWALLOW);
    f.settle(1);
    assert_eq!(f.store.get("grab").unwrap(), serde_json::json!({"modifiers": "CTRL_LEFT", "key": "K"}));
    let views = drain(&sub);
    assert!(views.iter().any(|v| v.event == Event::Saved && v.revision == 1));
    assert_eq!(f.mod_up(Mods::CTRL_L), SWALLOW);
    f.down(Key::Escape);
    drop(rec);
    f.mod_down(Mods::CTRL_L);
    assert_eq!(f.down(Key::K), SWALLOW);
    assert!(f.log.lock().unwrap().actions.contains(&(2, true)));
}

/// Cancel before the hook is ready: one final event, keys pass meanwhile, and a late arm does nothing.
#[test]
fn cancel_before_armed() {
    let f = fake();
    f.hold_arm.store(true, Ordering::Release);
    let owner = f.input.owner(window()).unwrap();
    let rec = owner.record("grab").unwrap();
    let sub = rec.observe();
    assert_eq!(f.down(Key::K), PASS, "Starting passes keys");
    f.up(Key::K);
    assert!(rec.cancel().try_get().is_some());
    f.arm_now();
    let views = drain(&sub);
    assert_eq!(views[0].phase, Phase::Starting);
    assert_eq!(terminals(&views), 1);
    assert_eq!(f.down(Key::Escape), PASS);
}

/// A recording that never gets its hook ready fails at the start deadline and opens the keyboard.
#[test]
fn a_start_that_never_arms_fails() {
    let f = fake();
    f.hold_arm.store(true, Ordering::Release);
    let owner = f.input.owner(window()).unwrap();
    let rec = owner.record("grab").unwrap();
    let sub = rec.observe();
    f.advance(Duration::from_secs(3));
    let views = drain(&sub);
    assert!(matches!(last_event(&views), Event::Failed { .. }));
    assert_eq!(f.down(Key::K), PASS);
}

/// Ctrl held before the recording began is part of the chord, and its release still passes.
#[test]
fn pre_held_modifiers_keep_their_release() {
    let f = fake();
    f.mod_down(Mods::CTRL_L);
    let owner = f.input.owner(window()).unwrap();
    let _rec = owner.record("grab").unwrap();
    assert_eq!(f.down(Key::K), SWALLOW);
    assert_eq!(f.up(Key::K), SWALLOW);
    f.settle(1);
    assert_eq!(f.store.get("grab").unwrap()["modifiers"], "CTRL_LEFT");
    assert_eq!(f.mod_up(Mods::CTRL_L), PASS);
}

/// Cancelling with a captured key held keeps swallowing that key's repeats and release, then lets everything through.
#[test]
fn a_captured_key_drains_after_cancel() {
    let f = fake();
    let owner = f.input.owner(window()).unwrap();
    let rec = owner.record("grab").unwrap();
    assert_eq!(f.down(Key::K), SWALLOW);
    rec.cancel();
    assert_eq!(f.down(Key::K), SWALLOW, "repeat");
    assert_eq!(f.up(Key::K), SWALLOW);
    assert_eq!(f.down(Key::K), PASS);
}

/// A bare Escape cancels even when a main key's release was lost.
#[test]
fn escape_cancels_after_a_lost_main_key_up() {
    let f = fake();
    let owner = f.input.owner(window()).unwrap();
    let rec = owner.record("grab").unwrap();
    let sub = rec.observe();
    f.mod_down(Mods::CTRL_L);
    f.down(Key::K);
    f.mod_up(Mods::CTRL_L);
    f.down(Key::Escape);
    assert_eq!(last_event(&drain(&sub)), Event::Cancelled { reason: Reason::User });
}

/// A click outside the registered regions cancels; one inside does not.
#[test]
fn an_outside_click_cancels() {
    let f = fake();
    let owner = f.input.owner(window()).unwrap();
    let rec = owner.record("grab").unwrap();
    let sub = rec.observe();
    rec.regions(vec![Fake::rect(0, 0, 100, 100)]);
    f.pointer(50, 50);
    assert_eq!(terminals(&drain(&sub)), 0);
    f.pointer(500, 500);
    assert_eq!(last_event(&drain(&sub)), Event::Cancelled { reason: Reason::Outside });
}

/// A start whose request was cancelled long ago is refused however many other cancels came since.
#[test]
fn a_stale_start_is_refused_after_many_cancels() {
    let f = fake();
    let owner = f.input.owner(window()).unwrap();
    for _ in 0..40 {
        owner.record("grab").unwrap().cancel();
    }
    let late = Hub::new(crate::input::RecorderView {
        lease: 3,
        seq: 0,
        phase: Phase::Starting,
        open: true,
        parts: vec![],
        live: false,
        held: false,
        error: None,
        saves: 0,
        revision: 0,
        event: Event::None,
    });
    let sub = late.subscribe();
    f.post(Command::Record { owner: owner.id, seq: 3, idx: 2, hub: late });
    assert_eq!(last_event(&drain(&sub)), Event::Cancelled { reason: Reason::Revoked });
    assert_eq!(f.down(Key::K), PASS, "nothing is capturing");
}

/// A replaced recording ends once as replaced; its held key drains and never reaches the new recording.
#[test]
fn a_replaced_recording_cannot_feed_the_new_one() {
    let f = fake();
    let owner = f.input.owner(window()).unwrap();
    let first = owner.record("grab").unwrap();
    let (s1,) = (first.observe(),);
    f.down(Key::K);
    let second = owner.record("selection").unwrap();
    let s2 = second.observe();
    assert_eq!(last_event(&drain(&s1)), Event::Cancelled { reason: Reason::Replaced });
    assert_eq!(f.up(Key::K), SWALLOW);
    assert!(drain(&s2).iter().all(|v| v.parts.is_empty()), "the old key's release did not reach the new recording");
    f.down(Key::Escape);
    assert_eq!(terminals(&drain(&s2)), 1);
}

/// Cancel never waits on a save: the keyboard opens at once, the final event carries the pending count, and a late success is a catalog change, not another recording event.
#[test]
fn cancel_does_not_wait_for_a_blocked_save() {
    let f = fake();
    f.store.block();
    let owner = f.input.owner(window()).unwrap();
    let rec = owner.record("grab").unwrap();
    let sub = rec.observe();
    f.mod_down(Mods::CTRL_L);
    f.down(Key::K);
    f.up(Key::K);
    assert!(rec.cancel().try_get().is_some());
    assert_eq!(f.down(Key::A), PASS);
    let views = drain(&sub);
    assert_eq!(views.last().unwrap().saves, 1);
    assert_eq!(terminals(&views), 1);
    f.store.unblock();
    f.settle(1);
    assert!(drain(&sub).is_empty(), "nothing is published after the final event");
    assert_eq!(
        f.input.shortcuts().try_next().map(|u| match u {
            crate::input::Update::Next(v) | crate::input::Update::Resnapshot(v) => v.revision,
        }),
        Some(1)
    );
}

/// Opening the tray popup ends a recording as replaced and takes Escape, Enter and the arrows; its keys are swallowed and repeat.
#[test]
fn the_tray_popup_outranks_the_recorder() {
    let f = fake();
    let owner = f.input.owner(window()).unwrap();
    let rec = owner.record("grab").unwrap();
    let sub = rec.observe();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let inbox = seen.clone();
    let popup = owner
        .tray_popup(Fake::rect(0, 0, 10, 10), move |k| {
            inbox.lock().unwrap().push(k);
            true
        })
        .unwrap();
    assert_eq!(last_event(&drain(&sub)), Event::Cancelled { reason: Reason::Replaced });
    assert_eq!(f.down(Key::Escape), SWALLOW);
    assert_eq!(f.down(Key::DownArrow), SWALLOW);
    assert_eq!(f.down(Key::DownArrow), SWALLOW);
    assert_eq!(f.up(Key::Escape), SWALLOW);
    assert_eq!(*seen.lock().unwrap(), vec![PopupKey::Escape, PopupKey::Down, PopupKey::Down]);
    drop(popup);
    f.pump();
    assert_eq!(f.down(Key::Escape), PASS);
}

/// A popup whose inbox is gone is revoked at the next key and the key passes.
#[test]
fn a_dead_popup_is_revoked() {
    let f = fake();
    let owner = f.input.owner(window()).unwrap();
    let _popup = owner.tray_popup(Fake::rect(0, 0, 10, 10), |_| false).unwrap();
    assert_eq!(f.down(Key::Escape), PASS);
}

/// Starting a recording lets go of a held shortcut before it is acknowledged.
#[test]
fn recording_start_releases_held_actions() {
    let f = fake();
    f.mod_down(Mods::CTRL_L);
    f.down(Key::Space);
    assert_eq!(f.active(), vec![0]);
    let owner = f.input.owner(window()).unwrap();
    let _rec = owner.record("grab").unwrap();
    assert!(f.active().is_empty());
}

/// A recording nobody touches expires after 60 seconds and opens the keyboard even for keys it held.
#[test]
fn an_idle_recording_expires() {
    let f = fake();
    let owner = f.input.owner(window()).unwrap();
    let rec = owner.record("grab").unwrap();
    let sub = rec.observe();
    f.down(Key::K);
    f.advance(Duration::from_secs(61));
    assert_eq!(last_event(&drain(&sub)), Event::Cancelled { reason: Reason::Expired });
    assert_eq!(f.up(Key::K), PASS, "expiry passes everything");
}
