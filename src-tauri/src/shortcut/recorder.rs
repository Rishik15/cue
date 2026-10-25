//! Purpose: The thread behind one shortcut recording, with a keyboard hook of its own the way Handy records. It only reports keys; the settings window decides when a combination is finished (`useShortcutRecorder`) and calls `shortcut_commit`.
//! Contents: spawn — runs the recording's hook, emits one `shortcut-live` per key event, and ends when the recording is finished, cancelled, replaced or too old, then drops it (which removes the hook) and clears a recording still marked as its own.

use std::sync::Arc;
use std::time::{Duration, Instant};

use handy_keys::{Error, KeyboardListener};
use tauri::{AppHandle, Emitter};

use super::keys::view;
use super::matching::Live;
use super::{sync_blocking, Engine};

/// How long a single `recv` waits before the thread checks whether its recording is still the current one.
const LOOK: Duration = Duration::from_millis(100);
/// A recording nobody finishes (the popup left open) ends here, so its polling hook does not outlive the user's attention.
const LONGEST: Duration = Duration::from_secs(60);

/// Whether this thread's recording is still the current one.
fn current(engine: &Engine, name: &'static str, serial: u64) -> bool {
    let s = engine.lock();
    s.serial == serial && s.recording == Some(name)
}

/// Clears the recording if it is still this thread's, so shortcuts work again; false when it had already ended.
pub(super) fn end(engine: &Engine, name: &'static str, serial: u64) -> bool {
    let mut s = engine.lock();
    if s.serial != serial || s.recording != Some(name) {
        return false;
    }
    s.recording = None;
    sync_blocking(&engine.blocking, &s);
    true
}

/// The listener lives only as long as the recording, so nothing polls while Cue sits idle; dropping it removes its hook.
pub(super) fn spawn(
    app: AppHandle,
    engine: Arc<Engine>,
    name: &'static str,
    serial: u64,
    listener: KeyboardListener,
) -> Result<(), String> {
    let (worker_app, worker_engine) = (app.clone(), engine.clone());
    let started = crate::guard::spawn("shortcut-record", 256 * 1024, move || {
        let (app, engine) = (worker_app, worker_engine);
        let begun = Instant::now();
        while current(&engine, name, serial) && begun.elapsed() < LONGEST {
            match listener.recv_timeout(LOOK) {
                // Every key event is reported as it arrives; the page builds the display and decides when the combination is complete, as Handy's recorder does.
                Ok(ev) => {
                    let hotkey = ev.as_hotkey().ok();
                    let parts = hotkey.as_ref().map(view).unwrap_or_default();
                    let _ = app.emit(
                        "shortcut-live",
                        Live {
                            name,
                            serial,
                            is_key_down: ev.is_key_down,
                            keyed: ev.key.is_some(),
                            hotkey: hotkey.as_ref().and_then(|h| serde_json::to_value(h).ok()),
                            parts,
                        },
                    );
                }
                Err(Error::Timeout) => {}
                Err(_) => break,
            }
        }
        drop(listener);
        if end(&engine, name, serial) {
            // Show the binding it still has, so the popup closes to the right chip.
            let parts = engine.lock().bindings.iter().find(|b| b.name == name).map(|b| view(&b.hotkey)).unwrap_or_default();
            let _ = app.emit("shortcut-done", super::matching::Done::cancelled(name, parts));
            // the page closes its popup
        }
    });
    started.map_err(|e| {
        end(&engine, name, serial);
        format!("Cue could not start listening for keys: {e}")
    })
}
