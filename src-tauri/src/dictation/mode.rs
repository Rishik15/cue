//! Purpose: What a hotkey press or release means for each dictation mode, as pure logic so it can be tested without the OS.
//! Contents: Mode — Hold, Toggle or Hold or Toggle; Action; decide — the table; confirm_stop — whether a stop that waited out the regrip window still stands.

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

/// A release only ends the recording if the key stayed up for the regrip window: a shift of finger pressure or a key bounce comes straight
/// back down. `superseded` is true when a newer stop is waiting. A stop from a press (Toggle) has no key to wait for.
pub fn confirm_stop(from_release: bool, key_down: bool, superseded: bool) -> bool {
    !superseded && !(from_release && key_down)
}
