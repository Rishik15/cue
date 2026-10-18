//! Purpose: What a press of the dictionary shortcut does, given what the popup is doing: start a lookup, close the popup, or nothing.
//! Contents: Phase — idle, looking up, popup open; Action / press_action — the decision, with a short debounce so a double tap does not close the popup that is opening.

use std::time::Duration;

/// A second press this soon after the one that started the lookup is a double tap, not a request to close.
const DEBOUNCE: Duration = Duration::from_millis(300);

pub const IDLE: u8 = 0;
pub const LOOKING: u8 = 1;
pub const OPEN: u8 = 2;

#[derive(Debug, PartialEq)]
pub enum Action {
    Start,
    Close,
    Ignore,
}

pub fn press_action(phase: u8, since_start: Duration) -> Action {
    match phase {
        IDLE => Action::Start,
        _ if since_start < DEBOUNCE => Action::Ignore,
        _ => Action::Close,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_press_when_idle_starts_a_lookup() {
        assert_eq!(press_action(IDLE, Duration::from_secs(60)), Action::Start);
    }

    #[test]
    fn a_double_tap_is_ignored_and_a_later_press_closes() {
        for phase in [LOOKING, OPEN] {
            assert_eq!(press_action(phase, Duration::from_millis(100)), Action::Ignore);
            assert_eq!(press_action(phase, Duration::from_millis(900)), Action::Close);
        }
    }
}
