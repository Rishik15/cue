//! Purpose: Runs what a shortcut does on a thread of its own, in the order the keys came. The hotkeys thread only matches keys and posts here, so a feature that takes a moment to start (loading settings, opening the microphone) never delays the next key event.
//! Contents: Actions — the sender the hotkeys thread posts to; start — the thread, blocked on its channel while idle; perform — one press or release routed to its feature through the `registry` table (a push-to-talk shortcut also tells the settings window its held state).

use std::sync::mpsc::{self, Sender};

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use super::registry::{spec, Act};

/// `(shortcut name, down)`.
pub type Actions = Sender<(&'static str, bool)>;

#[derive(Serialize, Clone)]
struct Held {
    name: &'static str,
    down: bool,
}

pub(super) fn start(app: &AppHandle) -> std::io::Result<Actions> {
    let (actions, inbox) = mpsc::channel::<(&'static str, bool)>();
    let app = app.clone();
    crate::guard::spawn("shortcut-actions", 256 * 1024, move || {
        while let Ok((name, down)) = inbox.recv() {
            // A bug in one feature must not end the thread: every later shortcut would do nothing until a restart.
            let _ = crate::guard::contain(|| perform(&app, name, down));
        }
    })?;
    Ok(actions)
}

fn perform(app: &AppHandle, name: &str, down: bool) {
    let Some(spec) = spec(name) else { return };
    match spec.act {
        // Push-to-talk: act on press and release, and let an open settings window show the held state.
        Act::Hold(run) => {
            let _ = app.emit("shortcut-held", Held { name: spec.name, down });
            run(app, down);
        }
        Act::Press(run) => {
            if down {
                run(app);
            }
        }
    }
}
