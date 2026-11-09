//! File: text/backend.rs
//! Purpose: Where one model request goes. `Backend` is the seam the edit pipeline asks through; `Sidecar` is the real adapter that talks to the llama.cpp server over HTTP.
//! Contents: Backend — ask for one answer; Sidecar — a lease on the server plus a watchdog that ends the server when the caller cancels; request — the HTTP call; messages — the chat turns of a task.
//! Why a watchdog thread: ureq cannot be interrupted from another thread, so a request nobody wants any more can only be cut off by ending the server it waits on. One thread per edit (not per unit) polls the cancel token while the edit runs and aborts the lease; the blocked request then fails at once.

use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::sync::LazyLock;
use std::time::Duration;

use serde_json::{json, Value};

use super::cancel::Cancel;
use super::engine::{Fail, POLL};
use super::server::{self, Aborter};
use crate::actions::Task;

static AGENT: LazyLock<ureq::Agent> = LazyLock::new(|| {
    ureq::Agent::config_builder()
        .timeout_recv_response(Some(Duration::from_secs(300)))
        .timeout_recv_body(Some(Duration::from_secs(300)))
        .build()
        .into()
});

/// Something that answers one unit of text for a task.
pub trait Backend {
    /// Asks for the answer to `text` under `task`, generating at most `max_tokens`. Returns the raw answer, `Cancelled` when `cancel` was set meanwhile, or the reason it failed.
    fn ask(&self, task: &Task, text: &str, max_tokens: usize, cancel: &Cancel) -> Result<String, Fail>;
}

/// The real backend: a lease on the running server, held for as long as this lives, and the watchdog that serves its cancel token.
pub struct Sidecar {
    lease: server::Lease,
    _watchdog: Option<Sender<()>>,
}

impl Sidecar {
    /// Wraps a lease. Inputs: the lease and the cancel token of the caller. Returns the backend; dropping it releases the lease and retires the watchdog.
    /// shortcut: if the watchdog thread cannot start, a cancel is only noticed between units; no upgrade needed, thread start failing is a dying process.
    pub fn new(lease: server::Lease, cancel: &Cancel) -> Self {
        let _watchdog = watch(lease.aborter(), cancel.clone());
        Self { lease, _watchdog }
    }
}

/// Starts the watchdog. Inputs: the handle that ends the server and the cancel token. Returns the sender whose drop retires the watchdog, or None when the thread could not start.
fn watch(aborter: Aborter, cancel: Cancel) -> Option<Sender<()>> {
    let (done, finished) = channel::<()>();
    let body = move || loop {
        match finished.recv_timeout(POLL) {
            Err(RecvTimeoutError::Timeout) if cancel.is_set() => return aborter.abort(),
            Err(RecvTimeoutError::Timeout) => {}
            _ => return,
        }
    };
    crate::guard::spawn("text-cancel", 64 * 1024, body).is_ok().then_some(done)
}

impl Backend for Sidecar {
    /// One HTTP request on the caller's thread; a failure after the cancel token was set is a cancel, not an error.
    fn ask(&self, task: &Task, text: &str, max_tokens: usize, cancel: &Cancel) -> Result<String, Fail> {
        request(&self.lease.conn, task, text, max_tokens).map_err(|e| if cancel.is_set() { Fail::Cancelled } else { Fail::Failed(e) })
    }
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
        .post(format!("http://127.0.0.1:{}/v1/chat/completions", conn.port))
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", conn.key))
        .send(body.to_string())
        .map_err(|e| format!("The text engine did not answer: {e}"))?;
    let json: Value =
        serde_json::from_reader(response.into_body().into_reader()).map_err(|e| format!("Unreadable answer from the text engine: {e}"))?;
    let choice = &json["choices"][0];
    if choice["finish_reason"] == "length" && max_tokens > 1 {
        return Err("This text is too long for the text model.".into());
    }
    Ok(choice["message"]["content"].as_str().unwrap_or_default().to_string())
}
