//! File: dictionary/phase.rs
//! Purpose: What a press of the Dictionary shortcut does, given what the Lookup is doing: start a Lookup, close its Card or Search window, or nothing.
//! Contents: IDLE / LOOKING / OPEN — the phases; Action / press_action — the decision, with a short debounce so a double tap does not close the window that is opening.

use std::time::Duration;

/// A second press this soon after the one that started the lookup is a double tap, not a request to close.
const DEBOUNCE: Duration = Duration::from_millis(300);

/// No Lookup is running.
pub const IDLE: u8 = 0;
/// A Lookup is reading the selection.
pub const LOOKING: u8 = 1;
/// A Card or Search window is open.
pub const OPEN: u8 = 2;

/// What a press does.
#[derive(Debug, PartialEq)]
pub enum Action {
    Start,
    Close,
    Ignore,
}

/// Decides what a press does. Input: the phase and the time since the press that started the Lookup. Return: start, close or ignore.
pub fn press_action(phase: u8, since_start: Duration) -> Action {
    match phase {
        IDLE => Action::Start,
        _ if since_start < DEBOUNCE => Action::Ignore,
        _ => Action::Close,
    }
}
