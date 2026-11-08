//! File: input/tests/popup.rs
//! Purpose: Protect the tray popup surface through the public interface: a replaced popup is told to close, a popup that loses its lease is told to close, and Escape reaches the popup even when the keyboard still counts an earlier Escape as down.
//! Contents: scenario tests of `Owner::tray_popup` with the scripted keyboard.

use std::sync::{Arc, Mutex};

use super::fake;
use crate::input::core::Verdict;
use crate::input::fake::Fake;
use crate::input::{Key, PopupKey};

/// A popup inbox that records what it is told.
fn inbox() -> (Arc<Mutex<Vec<PopupKey>>>, impl Fn(PopupKey) -> bool + Send + 'static) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let sink = seen.clone();
    (seen, move |k| {
        sink.lock().unwrap().push(k);
        true
    })
}

/// A second popup replacing the first tells the first to close, so no window stays on screen without a lease.
#[test]
fn a_replaced_popup_is_told_to_close() {
    let f = fake();
    let owner = f.input.owner();
    let (first, first_inbox) = inbox();
    let _a = owner.tray_popup(Fake::rect(0, 0, 10, 10), first_inbox).unwrap();
    let (second, second_inbox) = inbox();
    let _b = owner.tray_popup(Fake::rect(0, 0, 10, 10), second_inbox).unwrap();
    assert_eq!(*first.lock().unwrap(), vec![PopupKey::Dismiss]);
    assert!(second.lock().unwrap().is_empty());
}

/// When a part a surface needs cannot be made, the popup is told to close before its lease is dropped.
#[test]
fn a_failed_surface_closes_the_popup() {
    let f = fake();
    let owner = f.input.owner();
    let (seen, popup_inbox) = inbox();
    let _popup = owner.tray_popup(Fake::rect(0, 0, 10, 10), popup_inbox).unwrap();
    f.fail_surfaces("the mouse hook could not be made");
    assert_eq!(*seen.lock().unwrap(), vec![PopupKey::Dismiss]);
}

/// An Escape the keyboard still counts as down (its release was missed, or it was held while the menu opened) still closes the menu on the next press.
#[test]
fn a_stale_escape_still_reaches_the_popup() {
    let f = fake();
    assert_eq!(f.down(Key::Escape), Verdict::Pass);
    let owner = f.input.owner();
    let (seen, popup_inbox) = inbox();
    let _popup = owner.tray_popup(Fake::rect(0, 0, 10, 10), popup_inbox).unwrap();
    assert_eq!(f.down(Key::Escape), Verdict::Swallow);
    assert_eq!(*seen.lock().unwrap(), vec![PopupKey::Escape]);
}
