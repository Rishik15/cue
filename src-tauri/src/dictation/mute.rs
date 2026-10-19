//! Purpose: Mute the speakers while dictating (setting "Mute Audio While Recording") and always give them back.
//! Contents: init — remember the app handle and undo a mute a crash left behind; start / stop — mute at the start of a recording, restore at its end.
//! Cue restores only what it muted: speakers already muted stay muted. The muted device is written to a file while held, so the
//! next launch can restore it if Cue died in between. A file, not a setting: saving a setting starts the store plugin's async runtime (8 idle threads).

use std::sync::{Mutex, OnceLock};

use std::path::PathBuf;

use tauri::{AppHandle, Manager};

use crate::{platform, settings, status};

const MARKER: &str = "muted.endpoint";

fn marker(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_data_dir().ok().map(|dir| dir.join(MARKER))
}

static APP: OnceLock<AppHandle> = OnceLock::new();
/// The device Cue is holding muted. The lock also orders `start` and `stop`, which run on their own threads.
static HELD: Mutex<Option<String>> = Mutex::new(None);

pub fn init(app: &AppHandle) {
    let _ = APP.set(app.clone());
    let Some(id) = marker(app).and_then(|m| std::fs::read_to_string(m).ok()).filter(|id| !id.is_empty()) else { return };
    spawn("unmute", move |app| {
        restore(app, &id);
    }); // a previous run was killed while muting
}

fn spawn(name: &str, job: impl FnOnce(&AppHandle) + Send + 'static) {
    let Some(app) = APP.get().cloned() else { return };
    let _ = crate::guard::spawn(name, 256 * 1024, move || job(&app));
}

/// True when the speakers are back (or the marker is gone); false leaves the marker so the next launch or stop retries.
fn restore(app: &AppHandle, id: &str) -> bool {
    match platform::unmute_output(id) {
        Ok(()) => {
            if let Some(m) = marker(app) {
                let _ = std::fs::remove_file(m);
            }
            true
        }
        Err(e) => {
            status::report(app, &format!("Could not restore your speaker volume: {e}"));
            false
        }
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
                if let Some(m) = marker(app) {
                    let _ = std::fs::create_dir_all(m.parent().unwrap_or(&m)).and_then(|()| std::fs::write(m, &id));
                }
                *held = Some(id);
            }
            Ok(None) => {}
            Err(e) => crate::status::report(app, &format!("Could not mute the speakers: {e}")),
        }
    });
}

/// Recording ended (released, cancelled or failed).
pub fn stop() {
    spawn("unmute", |app| {
        let mut held = HELD.lock().unwrap();
        let Some(id) = held.take() else { return };
        if !restore(app, &id) {
            *held = Some(id); // still muted: the next stop retries
        }
    });
}
