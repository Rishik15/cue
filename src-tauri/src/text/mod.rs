//! File: text/mod.rs
//! Purpose: The text model: turn a task and a text into a model-edited text, on the user's device. Runs llama.cpp's server only while a request needs it.
//! Contents: init — remember the app handle and register `run` as the answerer of model actions; run — one edit for an action, errors shown through `status::report`; prepare — start the server and warm its prompt cache ahead of a request;
//! stop_and_remove — end the server and delete the model's files. `catalog` pins the files, `engine` owns the server's life, `server` starts the real process, `backend` asks it, `pipeline` runs the unit loop, `chunk` splits the text into sentences or paragraphs, `reply` checks answers, `cancel` is the signal that a call is no longer wanted.
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
mod reply;
mod server;
mod spans;
mod verify;
mod window;

use std::sync::{Mutex, OnceLock};

use tauri::AppHandle;

use crate::actions::Task;
use crate::status;
use backend::SidecarBackend;
pub use cancel::Cancel;
use engine::Fail;
use pipeline::{Outcome, Pipeline};

static APP: OnceLock<AppHandle> = OnceLock::new();
/// One request at a time: the model has one slot, and two callers (a dictation and a selection) would only slow each other down.
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

/// Remembers the app handle and registers `run` as the answerer of model actions. Input: the app handle. Returns nothing.
pub fn init(app: &AppHandle) {
    let _ = APP.set(app.clone());
    crate::actions::set_runner(run);
}

/// Ends the server and deletes the model's files. Waits for a running request first, and refuses new calls until the files are gone, so a delete cannot cut one off or race a start.
pub fn stop_and_remove(app: &AppHandle, model: &'static catalog::TextModel) -> std::io::Result<()> {
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let _no_calls = server::stop_for_delete();
    catalog::remove(app, model)
}

/// Whether a text model is installed, so the palette only lists the AI actions when they can run.
pub fn installed() -> bool {
    APP.get().is_some_and(|app| catalog::current(app).is_some())
}

/// Runs `task` over `text`. A failure is shown to the user and returns None, so the caller keeps the original text; a notice (part of a long text was left as written) is shown on the pill.
pub fn run(task: &'static Task, text: &str) -> Option<String> {
    let app = APP.get()?;
    match edit(app, task, text) {
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
/// while the user is still speaking or the copy runs. The caller's cancel token (a palette request) is taken here and honoured by the start.
pub fn prepare(task: &'static Task) {
    let Some(app) = APP.get().cloned() else { return };
    let cancel = crate::palette::cancel_token();
    let _ = crate::guard::spawn("text-prepare", 256 * 1024, move || {
        let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        if !server::keeps_warm() {
            return;
        }
        let Some(model) = catalog::current(&app) else { return };
        let Ok(lease) = server::connect(model, &cancel) else { return };
        Pipeline::new(&SidecarBackend::new(lease, model, &cancel), &cancel).warm(task);
    });
}

/// One edit: takes the one-at-a-time lock, gets the server and runs the pipeline. Returns the edited text and an optional notice, or the message to show; an empty message means say nothing.
fn edit(app: &AppHandle, task: &'static Task, text: &str) -> Result<(String, Option<String>), String> {
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let cancel = crate::palette::cancel_token();
    if cancel.is_set() {
        return Err(String::new());
    }
    let model = catalog::current(app).ok_or("No text model. Download one in Settings, Models.")?;
    let lease = server::connect(model, &cancel).map_err(|fail| match fail {
        Fail::Cancelled => String::new(),
        Fail::Failed(why) => why,
    })?;
    match Pipeline::new(&SidecarBackend::new(lease, model, &cancel), &cancel).run(task, text) {
        Outcome::Edited { text, notice } => Ok((text, notice)),
        Outcome::NoUsableAnswer(reason) => Err(format!("The text model gave no usable answer (it {}), so your text was left as it was.", reason.explain())),
        Outcome::Cancelled => Err(String::new()),
        Outcome::Failed(why) => Err(why),
    }
}

#[cfg(test)]
mod tests;
