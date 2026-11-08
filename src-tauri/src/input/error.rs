//! File: input/error.rs
//! Purpose: Why an input request failed, in words the Shortcuts page can show.
//! Contents: InputError — the failure kinds with their user-readable text.

use std::fmt;

/// Why an input request failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputError {
    UnknownShortcut(String),
    InvalidBinding(String),
    Conflict(String),
    Reserved,
    Unavailable(String),
    Busy,
    Overloaded,
    Persistence(String),
    InjectionPartial { sent: usize, wanted: usize },
}

impl fmt::Display for InputError {
    /// Writes the message the page shows.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InputError::UnknownShortcut(id) => write!(f, "Unknown shortcut: {id}"),
            InputError::InvalidBinding(why) | InputError::Conflict(why) | InputError::Persistence(why) | InputError::Unavailable(why) => {
                f.write_str(why)
            }
            InputError::Reserved => f.write_str("Escape on its own is how Cue closes things, so it cannot be a shortcut."),
            InputError::Busy => f.write_str("Cue is still saving earlier shortcut changes. Try again in a moment."),
            InputError::Overloaded => f.write_str("Cue could not keep up with the keyboard and let the keys through."),
            InputError::InjectionPartial { sent, wanted } => write!(f, "Cue sent {sent} of {wanted} key events."),
        }
    }
}

impl std::error::Error for InputError {}
