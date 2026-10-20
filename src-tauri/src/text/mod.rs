//! Purpose: The text model: turn a task and a text into a model-edited text, on the user's device. Runs llama.cpp's server only while a request needs it.
//! Contents: init — remember the app handle and register `run` as the answerer of model actions; run — one edit for an action, errors shown through `status::report`; prepare — start the server and warm its prompt cache ahead of a request;
//! stop_and_remove — end the server and delete the model's files. `catalog` pins the files, `server` owns the process, `chunk` splits the text into sentences or paragraphs, `reply` checks answers.
//! Why it is fast and safe (all measured): the instruction and examples are one fixed prefix that the server keeps in its prompt cache, sampling is greedy, each sentence is a separate small request,
//! and an answer that strays (answers or obeys the text, loses a link or number) is dropped so that sentence stays as written.

pub mod catalog;
mod chunk;
#[cfg(test)]
mod props;
mod reply;
mod server;

use std::sync::{LazyLock, Mutex, OnceLock};
use std::time::Duration;

use serde_json::{json, Value};
use tauri::AppHandle;

use crate::actions::Task;
use crate::sanitize::strip_hidden;
use crate::status;
use chunk::Seg;

/// Longest text sent to the model. A longer one would keep the model busy for minutes; split it, or use a rule that needs no model.
pub(crate) const MAX_CHARS: usize = 20_000;

static AGENT: LazyLock<ureq::Agent> = LazyLock::new(|| ureq::AgentBuilder::new().timeout_read(Duration::from_secs(300)).build());
static APP: OnceLock<AppHandle> = OnceLock::new();
/// One request at a time: the model has one slot, and two callers (a dictation and a selection) would only slow each other down.
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

pub fn init(app: &AppHandle) {
    let _ = APP.set(app.clone());
    crate::actions::set_runner(run);
}

/// Ends the server and deletes the model's files. Waits for a running request first, so a delete cannot cut one off or race a start.
pub fn stop_and_remove(app: &AppHandle, model: &'static catalog::TextModel) -> std::io::Result<()> {
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    server::stop();
    catalog::remove(app, model)
}

/// Whether a text model is installed, so the palette only lists the AI actions when they can run.
pub fn installed() -> bool {
    APP.get().is_some_and(|app| catalog::current(app).is_some())
}

/// Runs `task` over `text`. A failure is shown to the user and returns None, so the caller keeps the original text.
pub fn run(task: &'static Task, text: &str) -> Option<String> {
    let app = APP.get()?;
    match edit(app, task, text) {
        Ok(out) => Some(out),
        Err(e) if e.is_empty() => None,
        Err(e) => {
            status::report(app, &e);
            None
        }
    }
}

/// Starts the server and sends the task's prefix (instruction and examples) with a one-token answer, so the real request finds both the model
/// loaded and the prefix cached. Called when a dictation or a selection begins, while the user is still speaking or the copy runs.
pub fn prepare(task: &'static Task) {
    let Some(app) = APP.get().cloned() else { return };
    let _ = crate::guard::spawn("text-prepare", 256 * 1024, move || {
        let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        if !server::keeps_warm(&app) {
            return; // it would stop again right after this
        }
        let Some(model) = catalog::current(&app) else { return };
        let Ok(lease) = server::connect(&app, model) else { return }; // the real request reports why
        let _ = request(&lease.conn, task, ".", 1);
    });
}

fn edit(app: &AppHandle, task: &'static Task, text: &str) -> Result<String, String> {
    if text.chars().count() > MAX_CHARS {
        return Err(format!("This text is too long for the text model ({MAX_CHARS} characters at most)."));
    }
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    if crate::palette::superseded() {
        return Err(String::new()); // the user already moved on to another row: nothing to run, nothing to report
    }
    let model = catalog::current(app).ok_or("No text model. Download one in Settings, Models.")?;
    let lease = server::connect(app, model)?;
    let segs = chunk::split(text, task.unit);
    let (mut answers, mut units, mut kept) = (Vec::new(), 0, 0);
    for seg in &segs {
        let Seg::Unit(unit) = seg else { continue };
        if crate::palette::superseded() {
            return Err(String::new()); // moved on mid-text: stop sending units
        }
        units += 1;
        // The model sees the unit without hidden characters (tag characters can smuggle instructions); if its answer is rejected, the original unit stays.
        let clean = strip_hidden(unit);
        let answer = request(&lease.conn, task, &clean, reply::max_tokens(&clean));
        let accepted = answer.as_ref().ok().and_then(|raw| reply::accept(&clean, raw, task.max_change));
        kept += usize::from(accepted.is_none());
        match (accepted, answer) {
            (Some(better), _) => answers.push(better),
            (None, Err(e)) => return Err(e),
            (None, Ok(_)) => answers.push(unit.to_string()), // a bad answer: this unit stays as written
        }
    }
    if units > 0 && kept == units {
        return Err("The text model gave no usable answer, so your text was left as it was.".into());
    }
    Ok(chunk::join(&segs, &answers))
}

/// The conversation: instruction, then each example as a user and an assistant turn, then the text. Everything before the text is the same for every request of a task.
fn messages(task: &Task, text: &str) -> Vec<Value> {
    let mut turns = vec![json!({ "role": "system", "content": task.system })];
    for (said, answered) in task.shots {
        turns.push(json!({ "role": "user", "content": said }));
        turns.push(json!({ "role": "assistant", "content": answered }));
    }
    turns.push(json!({ "role": "user", "content": text }));
    turns
}

/// Greedy decoding (temperature 0, top_k 1): an edit has one right answer, and measured better than temperature 0.2 or the model makers' sampling advice.
fn request(conn: &server::Conn, task: &Task, text: &str, max_tokens: usize) -> Result<String, String> {
    let body = json!({
        "messages": messages(task, text),
        "temperature": 0,
        "top_k": 1,
        "max_tokens": max_tokens,
        "cache_prompt": true,
        "stream": false,
    });
    let response = AGENT
        .post(&format!("http://127.0.0.1:{}/v1/chat/completions", conn.port))
        .set("Content-Type", "application/json")
        .set("Authorization", &format!("Bearer {}", conn.key))
        .send_string(&body.to_string())
        .map_err(|e| format!("The text engine did not answer: {e}"))?;
    let json: Value =
        serde_json::from_reader(response.into_reader()).map_err(|e| format!("Unreadable answer from the text engine: {e}"))?;
    let choice = &json["choices"][0];
    if choice["finish_reason"] == "length" && max_tokens > 1 {
        return Err("This text is too long for the text model.".into());
    }
    Ok(choice["message"]["content"].as_str().unwrap_or_default().to_string())
}
