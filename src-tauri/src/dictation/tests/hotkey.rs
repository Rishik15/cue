//! File: dictation/tests/hotkey.rs
//! Purpose: Protect what the dictation key does in each mode: Hold records while held, Toggle on and off, Hold or Toggle by how long the key was down, and a bounce never ends a recording.
//! Contents: tests of `mode::decide`, `Mode::parse` and `confirm_stop`.

use std::time::Duration;

use crate::dictation::mode::{confirm_stop, decide, Action, Mode, HOLD_AFTER};

const SHORT: Duration = Duration::from_millis(100);

/// Hold: pressing starts, releasing stops, and a repeated press while recording does nothing.
#[test]
fn hold_mode_records_only_while_the_key_is_down() {
    assert_eq!(decide(Mode::Hold, true, false, Duration::ZERO), Action::Start);
    assert_eq!(decide(Mode::Hold, true, true, Duration::ZERO), Action::Nothing);
    assert_eq!(decide(Mode::Hold, false, true, SHORT), Action::Stop);
    assert_eq!(decide(Mode::Hold, false, false, SHORT), Action::Nothing);
}

/// Toggle: one press starts, the next press stops, releases do nothing.
#[test]
fn toggle_mode_starts_and_stops_on_presses() {
    assert_eq!(decide(Mode::Toggle, true, false, Duration::ZERO), Action::Start);
    assert_eq!(decide(Mode::Toggle, false, true, HOLD_AFTER * 5), Action::Nothing);
    assert_eq!(decide(Mode::Toggle, true, true, Duration::ZERO), Action::Stop);
}

/// Hold or Toggle: a quick tap keeps recording until the next press; a long hold stops on release.
#[test]
fn hold_or_toggle_tells_a_tap_from_a_hold_by_how_long_the_key_was_down() {
    assert_eq!(decide(Mode::HoldOrToggle, false, true, SHORT), Action::Nothing);
    assert_eq!(decide(Mode::HoldOrToggle, true, true, Duration::ZERO), Action::Stop);
    assert_eq!(decide(Mode::HoldOrToggle, false, true, HOLD_AFTER), Action::Stop);
    assert_eq!(decide(Mode::HoldOrToggle, true, false, Duration::ZERO), Action::Start);
}

/// An unknown or missing saved mode means Hold, so an old or hand-edited settings file never leaves dictation without a working key.
#[test]
fn unknown_modes_fall_back_to_hold() {
    assert_eq!(Mode::parse(Some("Toggle")), Mode::Toggle);
    assert_eq!(Mode::parse(Some("Hold or Toggle")), Mode::HoldOrToggle);
    assert_eq!(Mode::parse(Some("Press twice")), Mode::Hold);
    assert_eq!(Mode::parse(None), Mode::Hold);
}

/// A release stands only if the key stayed up through the regrip window and no newer stop is waiting; a stop made by a press has no key to wait for.
#[test]
fn a_bounce_or_a_superseded_stop_does_not_end_the_recording() {
    assert!(confirm_stop(true, false, false));
    assert!(!confirm_stop(true, true, false));
    assert!(!confirm_stop(true, false, true));
    assert!(confirm_stop(false, true, false));
}
