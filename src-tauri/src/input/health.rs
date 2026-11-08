//! File: input/health.rs
//! Purpose: What Cue knows about the keyboard hook: installed but not yet seen working, seen working, being reinstalled, or failed. There is no probe key and no idle timer (ADR 0012).
//! Contents: Health — the four states and the first key that proves a hook works.

use serde::Serialize;

/// The state of the keyboard hook.
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Health {
    InstalledUnproven,
    Observed,
    Recovering,
    Failed,
}

impl Health {
    /// The state after a key arrived: only a hook that was installed or recovering becomes observed.
    pub fn after_key(self) -> Health {
        match self {
            Health::Failed => Health::Failed,
            _ => Health::Observed,
        }
    }
}
