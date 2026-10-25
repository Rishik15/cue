//! File: platform/win/recording.rs
//! Purpose: The keyboard listener a shortcut recording uses, behind the platform API so `shortcut` compiles on targets without the Windows-only constructor.
//! Contents: recording_listener — a listener of its own that swallows every key until it is dropped.

use handy_keys::KeyboardListener;

/// Starts the listener for one recording: its own hook, which swallows the keys pressed so they do nothing else.
/// Returns the listener, or a user-readable error when Windows refuses the hook.
pub fn recording_listener() -> Result<KeyboardListener, String> {
    KeyboardListener::new_recording().map_err(|e| format!("Cue could not listen for keys: {e}"))
}
