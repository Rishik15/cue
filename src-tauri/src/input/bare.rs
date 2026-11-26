//! File: input/bare.rs
//! Purpose: Input with no shortcuts, for the debug-only `cue --insert-test`: the same core, shared state and keyboard hook as the app, so `lift` sees the held modifiers and the epoch exactly as during a finished dictation, with nothing to match and nothing to run. Never part of the normal startup.
//! Contents: NoStore — no bindings, nothing saved; Discard — a sink that takes every effect and drops it; BARE — the empty catalog; start_bare — starts input over them.

use std::sync::Arc;

use super::api::Input;
use super::app::launch;
use super::catalog::Catalog;
use super::chord::Chord;
use super::effects::{Effect, Sink};
use super::error::InputError;
use super::save::Store;

/// A store with no bindings that refuses every write.
struct NoStore;

impl Store for NoStore {
    /// No shortcut has a stored value.
    fn load(&self, _: &str) -> Option<serde_json::Value> {
        None
    }

    /// Nothing is saved: the bare input never records a shortcut.
    fn save(&self, _: &str, _: &Chord) -> Result<(), String> {
        Err("The insert test keeps no shortcuts.".into())
    }

    /// Every switch reads on; there are no shortcuts to switch.
    fn enabled(&self, _: &str) -> bool {
        true
    }
}

/// Takes every effect and drops it: with no shortcuts there is no action to run.
struct Discard;

impl Sink for Discard {
    /// Accepts the effect and drops it.
    fn send(&mut self, _: Effect) -> bool {
        true
    }

    /// Nothing waits on this sink.
    fn wake(&mut self) {}
}

/// No shortcuts and no keyboard observers, so every key passes to the focused app.
pub(super) static BARE: Catalog = Catalog { specs: &[], on_escape: &[], on_key_down: &[], typing_wanted: || false, holds_escape: || false };

/// Starts input with no shortcuts: publishes the shared state `lift` reads and installs the keyboard hook that keeps it current. Returns the handles once the hook is installed, or the reason it could not be.
pub fn start_bare() -> Result<Input, InputError> {
    launch(&BARE, Arc::new(NoStore), |_| Ok(Box::new(Discard)))
}
