//! File: shortcut/events.rs
//! Purpose: The serial-tagged event stream a shortcut recording sends to the settings window. Every payload carries the recording's serial so the page can ignore events of any other recording.
//! Contents: Reason — why a recording was cancelled; Outcome / Done — the one final result of a recording; Live — the keys held now; Invalid — a combination refused while the recording stays open; Event — any of them, with `emit`.

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use super::keys::Part;

/// Why a recording was cancelled.
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum Reason {
    /// Escape on its own was pressed.
    User,
    /// The page cancelled it: a click outside the control, leaving the page, or resetting the shortcut.
    Outside,
    /// Nobody finished it within the time limit.
    Expired,
    /// Another recording started.
    Replaced,
    /// The settings window closed.
    WindowClosed,
}

/// How a recording ended.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub(super) enum Outcome {
    Saved,
    Cancelled { reason: Reason },
    Failed { message: String },
}

/// The one final event of a recording; `parts` is the shortcut as it is now (the new one when saved), so the page can draw its chip.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub(super) struct Done {
    pub(super) name: &'static str,
    pub(super) serial: u64,
    #[serde(flatten)]
    pub(super) outcome: Outcome,
    pub(super) parts: Vec<Part>,
}

/// The keys the recording shows right now; `keyed` means a main key took part, not a modifier alone.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub(super) struct Live {
    pub(super) name: &'static str,
    pub(super) serial: u64,
    pub(super) parts: Vec<Part>,
    pub(super) keyed: bool,
}

/// A combination Cue refused (a clash, or a key that needs a modifier); the recording stays open for another try.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub(super) struct Invalid {
    pub(super) name: &'static str,
    pub(super) serial: u64,
    pub(super) message: String,
}

/// Anything a recording tells the page.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Event {
    Live(Live),
    Invalid(Invalid),
    Done(Done),
}

impl Event {
    /// Sends the event to the windows under its own name (`shortcut-live`, `shortcut-invalid`, `shortcut-done`); a window that is gone just misses it.
    pub(super) fn emit(&self, app: &AppHandle) {
        let _ = match self {
            Event::Live(e) => app.emit("shortcut-live", e),
            Event::Invalid(e) => app.emit("shortcut-invalid", e),
            Event::Done(e) => app.emit("shortcut-done", e),
        };
    }
}
