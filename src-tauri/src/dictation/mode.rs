//! Purpose: What a hotkey press or release means for each dictation mode, as pure logic so it can be tested without the OS.
//! Contents: Mode — Hold, Toggle or Hold or Toggle; Action; decide — the table; live_from_start — whether typing happens as it is said; confirm_stop — whether a stop that waited out the regrip window still stands.

use std::time::Duration;

/// In Hold or Toggle, holding this long makes the key a hold (stop on release); a shorter tap leaves recording on until the next press.
pub const HOLD_AFTER: Duration = Duration::from_millis(300);

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Mode {
    Hold,
    Toggle,
    HoldOrToggle,
}

#[derive(PartialEq, Debug)]
pub enum Action {
    Start,
    Stop,
    Nothing,
}

impl Mode {
    /// Unknown or missing setting values mean Hold, the plain default.
    pub fn parse(setting: Option<&str>) -> Mode {
        match setting {
            Some("Toggle") => Mode::Toggle,
            Some("Hold or Toggle") => Mode::HoldOrToggle,
            _ => Mode::Hold,
        }
    }
}

/// `held` is how long the key was down when this release happened.
pub fn decide(mode: Mode, down: bool, recording: bool, held: Duration) -> Action {
    match (down, recording, mode) {
        (true, false, _) => Action::Start,
        (true, true, Mode::Toggle | Mode::HoldOrToggle) => Action::Stop,
        (false, true, Mode::Hold) => Action::Stop,
        (false, true, Mode::HoldOrToggle) if held >= HOLD_AFTER => Action::Stop,
        _ => Action::Nothing,
    }
}

/// A toggled recording has no key held, so its sentences are typed as they come. Hold to talk types once, on release. Hold or Toggle
/// does not know until the key comes up: a tap makes it a toggle.
pub fn live_from_start(mode: Mode) -> bool {
    mode == Mode::Toggle
}

/// A release only ends the recording if the key stayed up for the regrip window: a shift of finger pressure or a key bounce comes straight
/// back down. `superseded` is true when a newer stop is waiting. A stop from a press (Toggle) has no key to wait for.
pub fn confirm_stop(from_release: bool, key_down: bool, superseded: bool) -> bool {
    !superseded && !(from_release && key_down)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TAP: Duration = Duration::from_millis(120);
    const LONG: Duration = Duration::from_millis(900);

    #[test]
    fn hold_records_while_held() {
        assert_eq!(decide(Mode::Hold, true, false, TAP), Action::Start);
        assert_eq!(decide(Mode::Hold, false, true, LONG), Action::Stop);
        assert_eq!(decide(Mode::Hold, true, true, TAP), Action::Nothing);
    }

    #[test]
    fn toggle_flips_on_each_press_and_ignores_releases() {
        assert_eq!(decide(Mode::Toggle, true, false, TAP), Action::Start);
        assert_eq!(decide(Mode::Toggle, false, true, LONG), Action::Nothing);
        assert_eq!(decide(Mode::Toggle, true, true, TAP), Action::Stop);
    }

    #[test]
    fn hold_or_toggle_tells_a_tap_from_a_hold() {
        assert_eq!(decide(Mode::HoldOrToggle, false, true, LONG), Action::Stop);
        assert_eq!(decide(Mode::HoldOrToggle, false, true, TAP), Action::Nothing); // tap: keeps listening
        assert_eq!(decide(Mode::HoldOrToggle, true, true, TAP), Action::Stop); // the next press ends it
        assert_eq!(decide(Mode::Hold, false, false, LONG), Action::Nothing);
    }

    #[test]
    fn a_release_followed_by_a_press_is_a_regrip() {
        assert!(confirm_stop(true, false, false));
        assert!(!confirm_stop(true, true, false)); // pressed again: keep recording
        assert!(confirm_stop(false, true, false)); // Toggle's stop is itself a press
        assert!(!confirm_stop(true, false, true)); // a newer stop owns it
    }

    #[test]
    fn only_toggle_types_live_from_the_start() {
        assert!(live_from_start(Mode::Toggle));
        assert!(!live_from_start(Mode::Hold));
        assert!(!live_from_start(Mode::HoldOrToggle));
    }

    #[test]
    fn unknown_setting_is_hold() {
        assert_eq!(Mode::parse(Some("Toggle")), Mode::Toggle);
        assert_eq!(Mode::parse(Some("Hold or Toggle")), Mode::HoldOrToggle);
        assert_eq!(Mode::parse(Some("whatever")), Mode::Hold);
        assert_eq!(Mode::parse(None), Mode::Hold);
    }
}
