//! File: text/mod.rs
//! Purpose: The text model: turn a task and a text into a model-edited text, on the user's device. Runs llama.cpp's server only while a request needs it.
//! Contents: init — remember the app handle and register `run` as the answerer of model actions; run — one edit for an action, errors shown through `status::report`; prepare — start the server and warm its prompt cache ahead of a request; warm_up — the turn-taking part of `prepare`;
//! idle_changed — apply a changed idle setting now; stop_and_remove — end the server and delete the model's files. `catalog` pins the files, `engine` owns the server's life, `server` starts the real process, `backend` asks it, `pipeline` runs the unit loop, `chunk` splits the text into sentences or paragraphs, `reply` checks answers, `cancel` is the signal that a call is no longer wanted, `turn` lets one request run at a time.
//! Why it is fast and safe (all measured): the instruction and examples are one fixed prefix that the server keeps in its prompt cache, sampling is greedy, each sentence is a separate small request,
//! and an answer that strays (answers or obeys the text, loses a link or number) is dropped so that sentence stays as written.

mod backend;
mod budget;
mod cancel;
pub mod catalog;
mod chunk;
mod engine;
mod mask;
mod pipeline;
pub(crate) mod reply;
mod server;
mod spans;
mod turn;
mod verify;
mod window;

use std::sync::OnceLock;

use tauri::AppHandle;

use crate::actions::Task;
use crate::status;
use backend::SidecarBackend;
pub use cancel::Cancel;
use engine::Fail;
use pipeline::{Outcome, Pipeline};
use turn::Turn;

static APP: OnceLock<AppHandle> = OnceLock::new();
/// One request at a time: the model has one slot, and two callers (a dictation and a selection) would only slow each other down.
static ONE_AT_A_TIME: Turn = Turn::new();

/// Remembers the app handle and registers `run` as the answerer of model actions. Input: the app handle. Returns nothing.
pub fn init(app: &AppHandle) {
    let _ = APP.set(app.clone());
    crate::actions::set_runner(run);
}

/// Ends the server and deletes the model's files. Refuses new calls and abandons a start that is still loading at once, waits for a running request, and keeps refusing until the files are gone, so a delete cannot cut one off or race a start.
pub fn stop_and_remove(app: &AppHandle, model: &'static catalog::TextModel) -> std::io::Result<()> {
    let no_calls = server::refuse_calls();
    let _one = ONE_AT_A_TIME.take(&Cancel::never());
    no_calls.stop();
    catalog::remove(app, model)
}

/// Applies a changed "unload after" setting to a loaded server at once.
pub fn idle_changed() {
    server::idle_changed();
}

/// Whether a text model is installed, so the palette only lists the AI actions when they can run.
pub fn installed() -> bool {
    APP.get().is_some_and(|app| catalog::current(app).is_some())
}

/// Runs `task` over `text`. A failure is shown to the user and returns None, so the caller keeps the original text; a notice (part of a long text was left as written) is shown on the pill.
pub fn run(task: &'static Task, text: &str) -> Option<String> {
    let app = APP.get()?;
    match edit(app, task, text, &crate::palette::cancel_token()) {
        Ok((out, notice)) => {
            if let Some(line) = notice {
                let prefs = crate::prefs::Prefs::load();
                crate::platform::overlay::notice(&line, prefs.overlay_top, prefs.theme());
            }
            Some(out)
        }
        Err(e) if e.is_empty() => None,
        Err(e) => {
            status::report(app, &e);
            None
        }
    }
}

/// Starts the server and warms the task's prompt cache, so the real request finds both the model loaded and the prefix cached. Called when a dictation or a selection begins,
/// while the user is still speaking or the copy runs. The caller's `cancel` is honoured by the wait for the turn, the start and the warm-up.
pub fn prepare(task: &'static Task, cancel: Cancel) {
    let Some(app) = APP.get().cloned() else { return };
    let _ = crate::guard::spawn("text-prepare", 256 * 1024, move || {
        warm_up(&ONE_AT_A_TIME, &cancel, |cancel| {
            if !server::keeps_warm() {
                return;
            }
            let Some(model) = catalog::current(&app) else { return };
            let Ok(lease) = server::connect(model, cancel) else { return };
            Pipeline::new(&SidecarBackend::new(lease, model, cancel), cancel).warm(task);
        });
    });
}

/// Waits for `turn` (leaving the queue when `cancel` is set), then runs `start` with the same token while holding it. Inputs: the turn, the caller's token and the start-and-warm step. Returns nothing; the turn is free again when it returns.
fn warm_up(turn: &Turn, cancel: &Cancel, start: impl FnOnce(&Cancel)) {
    if let Some(_one) = turn.take(cancel) {
        start(cancel);
    }
}

/// One edit: waits its turn (leaving the queue when `cancel` is set), gets the server and runs the pipeline. Inputs: the app, the task, the text and the caller's token. Returns the edited text and an optional notice, or the message to show; an empty message means say nothing.
fn edit(app: &AppHandle, task: &'static Task, text: &str, cancel: &Cancel) -> Result<(String, Option<String>), String> {
    let Some(_one) = ONE_AT_A_TIME.take(cancel) else { return Err(String::new()) };
    if cancel.is_set() {
        return Err(String::new());
    }
    let model = catalog::current(app).ok_or("No text model. Download one in Settings, Models.")?;
    let lease = server::connect(model, cancel).map_err(|fail| match fail {
        Fail::Cancelled => String::new(),
        Fail::Failed(why) => why,
    })?;
    match Pipeline::new(&SidecarBackend::new(lease, model, cancel), cancel).run(task, text) {
        Outcome::Edited { text, notice } => Ok((text, notice)),
        Outcome::NoUsableAnswer(reason) => {
            Err(format!("The text model gave no usable answer (it {}), so your text was left as it was.", reason.explain()))
        }
        Outcome::Cancelled => Err(String::new()),
        Outcome::Failed(why) => Err(why),
    }
}

#[cfg(test)]
mod tests;
