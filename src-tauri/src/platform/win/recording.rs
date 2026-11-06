//! File: platform/win/recording.rs
//! Purpose: The keyboard listener a shortcut recording uses, behind the platform API so `shortcut` compiles on targets without the Windows-only constructor. One listener is made when Cue starts and rests (passing every key) until a recording arms it: a hook made on the spot hears nothing for its first moments.
//! Contents: warm_recording_listener — make the resting listener at startup; RecordingKeys — the armed listener a recording reads (events, whether a lock or resume interrupted it, release); recording_listener — arm it for one recording.

use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::Duration;

use handy_keys::{Error, KeyEvent, KeyboardListener};

static RESIDENT: OnceLock<Mutex<KeyboardListener>> = OnceLock::new();

/// Makes the resting recording listener once, at startup. A failure is logged; the first recording then tries again.
pub fn warm_recording_listener() {
    match KeyboardListener::new_resident_recording() {
        Ok(listener) => {
            let _ = RESIDENT.set(Mutex::new(listener));
        }
        Err(e) => crate::log::note(&format!("shortcut recording: the resting keyboard listener did not start: {e}")),
    }
}

/// The listener a recording reads from, armed for as long as the recording lasts and resting again when it is dropped.
pub struct RecordingKeys {
    listener: &'static Mutex<KeyboardListener>,
    token: u64,
}

impl RecordingKeys {
    /// The listener, whoever used it last having finished or panicked.
    fn listener(&self) -> MutexGuard<'_, KeyboardListener> {
        self.listener.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Waits up to `wait` for the next key event. Errors with `Timeout` when none came and with another error when the listener stopped or a lock or resume interrupted the recording.
    pub fn recv_timeout(&self, wait: Duration) -> Result<KeyEvent, Error> {
        let listener = self.listener();
        if listener.interrupted() {
            return Err(Error::EventLoopNotRunning);
        }
        listener.recv_timeout(wait)
    }

    /// Stops swallowing keys at once, unless a newer recording has armed the listener since.
    pub fn release(&self) {
        self.listener().release_keyboard(self.token);
    }
}

impl Drop for RecordingKeys {
    /// Lets the keyboard go if the recording did not.
    fn drop(&mut self) {
        self.release();
    }
}

/// Arms the resting listener for one recording (it swallows the keys pressed so they do nothing else) and returns it.
/// Returns a user-readable error when no listener could be made or it did not answer.
pub fn recording_listener() -> Result<RecordingKeys, String> {
    if RESIDENT.get().is_none() {
        warm_recording_listener();
    }
    let resident = RESIDENT.get().ok_or("Cue could not listen for keys.")?;
    let token = resident.lock().unwrap_or_else(|e| e.into_inner()).arm();
    token
        .map(|token| RecordingKeys { listener: resident, token })
        .ok_or_else(|| "Cue could not start listening for keys. Close this and try again.".into())
}
