//! Purpose: Mute the speakers while dictating (setting "Mute Audio While Recording") and always give them back.
//! Contents: init — remember the app handle and undo a mute a crash left behind; start / stop — mute at the start of a recording, restore at its end.
//! Cue restores only what it muted: speakers already muted stay muted. The muted device is written to the settings store while held, so the
//! next launch can restore it if Cue died in between.

use std::sync::{Mutex, OnceLock};

use serde_json::Value;
use tauri::AppHandle;

use crate::{platform, settings, status};

const KEY: &str = "muted_endpoint";

static APP: OnceLock<AppHandle> = OnceLock::new();
/// The device Cue is holding muted. The lock also orders `start` and `stop`, which run on their own threads.
static HELD: Mutex<Option<String>> = Mutex::new(None);

pub fn init(app: &AppHandle) {
    let _ = APP.set(app.clone());
    let Some(id) = settings::get_str(app, KEY).filter(|id| !id.is_empty()) else { return };
    spawn("unmute", move |app| restore(app, &id)); // a previous run was killed while muting
}

fn spawn(name: &str, job: impl FnOnce(&AppHandle) + Send + 'static) {
    let Some(app) = APP.get().cloned() else { return };
    let _ = std::thread::Builder::new().name(name.into()).stack_size(256 * 1024).spawn(move || job(&app));
}

fn restore(app: &AppHandle, id: &str) {
    match platform::unmute_output(id) {
        Ok(()) => settings::put(app, KEY, Value::String(String::new())),
        Err(e) => status::report(app, &format!("Could not restore your speaker volume: {e}")),
    }
}

/// Recording began. A thread of its own, never the hotkey thread: talking to the audio service takes a few milliseconds.
pub fn start() {
    spawn("mute", |app| {
        if !settings::get_bool(app, "mute_while_recording", true) {
            return;
        }
        let mut held = HELD.lock().unwrap();
        // The recording may already be over by the time this thread runs.
        if held.is_some() || !super::is_recording() {
            return;
        }
        match platform::mute_output() {
            Ok(Some(id)) => {
                settings::put(app, KEY, Value::String(id.clone()));
                *held = Some(id);
            }
            Ok(None) => {}
            Err(e) => eprintln!("cue: could not mute the speakers: {e}"),
        }
    });
}

/// Recording ended (released, cancelled or failed).
pub fn stop() {
    spawn("unmute", |app| {
        if let Some(id) = HELD.lock().unwrap().take() {
            restore(app, &id);
        }
    });
}
