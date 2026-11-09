//! File: model_download/sink.rs
//! Purpose: The seam between a download and the windows: where progress, the end of a download and table changes are announced. The production adapter is `TauriSink`; tests plug in a recorder.
//! Contents: Progress / Finished — the event payloads; Sink — the trait; TauriSink — emits `model-progress`, `model-finished`, `model-changed`.

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use super::state::Outcome;
use crate::status;

/// The percent of a download moved: bytes done of the total.
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct Progress {
    pub id: &'static str,
    pub generation: u64,
    pub done: u64,
    pub total: u64,
}

/// A download ended, at table revision `revision`, with `outcome` and (when it failed) the message for the user.
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct Finished {
    pub id: &'static str,
    pub generation: u64,
    pub revision: u64,
    pub outcome: Outcome,
    pub error: Option<String>,
}

/// Where a download tells the world what happens to it.
pub trait Sink: Send + Sync {
    /// The percent of a download changed.
    fn progress(&self, event: Progress);

    /// A download ended; a failure carries its message for the user.
    fn finished(&self, event: Finished);

    /// The download table is at `revision` (a download started, changed phase or a delete took place).
    fn changed(&self, revision: u64);
}

/// Emits the events to every window and reports failures in the app.
pub struct TauriSink(pub AppHandle);

impl Sink for TauriSink {
    /// Sends `model-progress`.
    fn progress(&self, event: Progress) {
        let _ = self.0.emit("model-progress", event);
    }

    /// Reports a failure to the user, then sends `model-finished`.
    fn finished(&self, event: Finished) {
        if let Some(e) = &event.error {
            status::report_in_app(&self.0, e);
        }
        let _ = self.0.emit("model-finished", event);
    }

    /// Sends `model-changed`.
    fn changed(&self, revision: u64) {
        let _ = self.0.emit("model-changed", serde_json::json!({ "revision": revision }));
    }
}
