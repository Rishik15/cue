//! File: shortcut/recorder.rs
//! Purpose: The thread behind one shortcut recording: reads key events from a source, lets `gesture` say what they mean, and reports through `session`. Keeps recording after a save, so the next keys replace the combination; ends with exactly one final event however it stops.
//! Contents: KeySource / Pull — where key events come from (the real adapter is the recording hook, `KeyboardListener`; a test feeds a script); Limits — how often it looks up and how long it runs; run — the loop; spawn — run it on a named thread with the source dropped (and its hook removed) at the end.

use std::sync::Arc;
use std::time::{Duration, Instant};

use handy_keys::{Error, KeyEvent};
use tauri::AppHandle;

use crate::platform::RecordingKeys;

use super::events::{Event, Invalid, Live, Outcome, Reason};
use super::gesture::{Gesture, Step};
use super::session::{commit, current, end, with_host, Host, Verdict};
use super::Engine;

/// What a source gave when asked for the next key event.
pub(super) enum Pull {
    Event(KeyEvent),
    /// Nothing arrived within the wait.
    Idle,
    /// The source stopped for good.
    Closed,
}

/// Where a recording's key events come from. One real implementation (the recording hook) and one in a test.
pub(super) trait KeySource {
    /// Waits up to `wait` for the next key event.
    fn pull(&self, wait: Duration) -> Pull;

    /// Stops swallowing keys at once; the source is dropped a moment later.
    fn release(&self) {}
}

impl KeySource for RecordingKeys {
    /// Reads the hook's channel for up to `wait`.
    fn pull(&self, wait: Duration) -> Pull {
        match self.recv_timeout(wait) {
            Ok(ev) => Pull::Event(ev),
            Err(Error::Timeout) => Pull::Idle,
            Err(_) => Pull::Closed,
        }
    }

    /// Opens the keyboard again at the hook's next key.
    fn release(&self) {
        RecordingKeys::release(self);
    }
}

/// How a recording is paced: `look` is how long one wait for a key lasts before the loop checks the recording is still the running one, `longest` ends a recording nobody touches for that long.
pub(super) struct Limits {
    pub(super) look: Duration,
    pub(super) longest: Duration,
}

pub(super) const LIMITS: Limits = Limits { look: Duration::from_millis(100), longest: Duration::from_secs(60) };

/// Runs recording `serial` of `name` until it ends, and makes sure it ends with one final event: a cancelled recording, a source that stopped (`failed`), or `expired`. If something else already ended it (cancel, replace, window closed), that already sent the final event and this just returns.
pub(super) fn run(host: &Host, source: &dyn KeySource, name: &'static str, serial: u64, limits: &Limits) {
    let mut gesture = Gesture::default();
    let mut begun = Instant::now();
    while current(host.engine, name, serial) {
        if begun.elapsed() >= limits.longest {
            source.release();
            end(host, name, serial, Outcome::Cancelled { reason: Reason::Expired });
            return;
        }
        match source.pull(limits.look) {
            Pull::Idle => {}
            Pull::Closed => {
                let message = "Cue stopped hearing the keyboard. Close this and try again.".to_string();
                source.release();
                end(host, name, serial, Outcome::Failed { message });
                return;
            }
            Pull::Event(ev) => {
                begun = Instant::now();
                if !apply(host, &mut gesture, &ev, name, serial) {
                    source.release();
                    return;
                }
            }
        }
    }
    source.release();
}

/// Acts on one key event. Returns false when the recording is over.
fn apply(host: &Host, gesture: &mut Gesture, ev: &KeyEvent, name: &'static str, serial: u64) -> bool {
    match gesture.feed(ev) {
        Step::Ignore => true,
        Step::Live { parts, keyed } => {
            (host.emit)(Event::Live(Live { name, serial, parts, keyed }));
            true
        }
        Step::Cancel => {
            end(host, name, serial, Outcome::Cancelled { reason: Reason::User });
            false
        }
        Step::Commit(h) => match commit(host, name, serial, h) {
            Verdict::Saved => {
                gesture.reset();
                true
            }
            Verdict::Gone => false,
            Verdict::Refused(message) => {
                crate::log::note(&format!("record {name}: not accepted, {message}"));
                (host.emit)(Event::Invalid(Invalid { name, serial, message }));
                gesture.reset();
                true
            }
        },
    }
}

/// Runs the recording on a thread of its own. The source lives only as long as the recording, so nothing polls while Cue sits idle; dropping it removes its hook.
/// Returns an error when the thread cannot start.
pub(super) fn spawn(
    app: AppHandle,
    engine: Arc<Engine>,
    name: &'static str,
    serial: u64,
    source: impl KeySource + Send + 'static,
) -> Result<(), String> {
    crate::guard::spawn("shortcut-record", 256 * 1024, move || {
        with_host(&app, &engine, |host| run(host, &source, name, serial, &LIMITS));
        drop(source);
    })
    .map_err(|e| format!("Cue could not start listening for keys: {e}"))
}
