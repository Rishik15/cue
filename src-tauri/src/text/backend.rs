//! File: text/backend.rs
//! Purpose: Where one model request goes. `Backend` is the seam the edit pipeline asks through; `SidecarBackend` is the real adapter that talks to the llama.cpp server over HTTP.
//! Contents: Backend — ask for one answer, and size a request (context, token count, prefix tokens); tokenize — the server's tokenizer; SidecarBackend — a lease on the server plus a watchdog that ends the server when the caller cancels; request — the HTTP call; messages — the chat turns of a task.
//! Why a watchdog thread: ureq cannot be interrupted from another thread, so a request nobody wants any more can only be cut off by ending the server it waits on. One thread per edit (not per unit) polls the cancel token while the edit runs and aborts the lease; the blocked request then fails at once.

use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use serde_json::{json, Value};

use super::budget;
use super::cancel::Cancel;
use super::catalog::TextModel;
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
    /// The model's context in tokens, shared by the task's prefix, the text and the answer.
    fn context(&self) -> usize;

    /// The tokens in `text` by the model's own tokenizer, or None when it cannot count (the estimate is used then).
    fn count(&self, _text: &str) -> Option<usize> {
        None
    }

    /// The tokens of the task's fixed prefix (instruction and examples). The default is the safe estimate.
    fn prefix_tokens(&self, task: &Task) -> usize {
        budget::estimate(&budget::prefix_text(task))
    }

    /// Asks for the answer to `text` under `task`, generating at most `max_tokens`. Returns the raw answer, `Cancelled` when `cancel` was set meanwhile, or the reason it failed.
    fn ask(&self, task: &Task, text: &str, max_tokens: usize, cancel: &Cancel) -> Result<String, Fail>;
}

/// The real backend: a lease on the running server, held for as long as this lives, and the watchdog that serves its cancel token. The watchdog is declared first so it is dropped first: a cancel can never reach a server nobody holds any more.
pub struct SidecarBackend {
    _watchdog: Option<Sender<()>>,
    lease: server::Lease,
    model: &'static TextModel,
}

impl SidecarBackend {
    /// Wraps a lease. Inputs: the lease, the model it runs and the cancel token of the caller. Returns the backend; dropping it releases the lease and retires the watchdog.
    /// shortcut: if the watchdog thread cannot start, a cancel is only noticed between units; no upgrade needed, thread start failing is a dying process.
    /// shortcut: the watchdog polls the token every 100 ms, only while an edit runs; upgrade: abort the HTTP call directly once the client offers a handle.
    pub fn new(lease: server::Lease, model: &'static TextModel, cancel: &Cancel) -> Self {
        let _watchdog = watch(lease.aborter(), cancel.clone());
        Self { _watchdog, lease, model }
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

/// Token counts of task prefixes, measured once per task and model: the prefix does not change between requests.
static PREFIXES: LazyLock<Mutex<HashMap<(usize, &'static str), usize>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

impl Backend for SidecarBackend {
    /// The loaded model's own context size.
    fn context(&self) -> usize {
        self.model.context as usize
    }

    /// Asks the server's tokenizer; None when it does not answer.
    fn count(&self, text: &str) -> Option<usize> {
        tokenize(&self.lease.conn, text)
    }

    /// The prefix's token count, from the server once per task and model, else the safe estimate.
    fn prefix_tokens(&self, task: &Task) -> usize {
        let key = (task as *const Task as usize, self.model.id);
        if let Some(known) = PREFIXES.lock().unwrap_or_else(|e| e.into_inner()).get(&key) {
            return *known;
        }
        let Some(counted) = self.count(&budget::prefix_text(task)) else { return budget::estimate(&budget::prefix_text(task)) };
        PREFIXES.lock().unwrap_or_else(|e| e.into_inner()).insert(key, counted);
        counted
    }

    /// One HTTP request on the caller's thread; a failure after the cancel token was set is a cancel, not an error.
    fn ask(&self, task: &Task, text: &str, max_tokens: usize, cancel: &Cancel) -> Result<String, Fail> {
        request(&self.lease.conn, task, text, max_tokens).map_err(|e| if cancel.is_set() { Fail::Cancelled } else { Fail::Failed(e) })
    }
}

/// Counts the tokens of `text` with the server's tokenizer. Inputs: how to reach the server and the text. Returns the count, or None when the server does not answer or answers something else.
fn tokenize(conn: &server::Conn, text: &str) -> Option<usize> {
    let response = AGENT
        .post(format!("http://127.0.0.1:{}/tokenize", conn.port))
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", conn.key))
        .send(json!({ "content": text }).to_string())
        .ok()?;
    let body: Value = serde_json::from_reader(response.into_body().into_reader()).ok()?;
    body["tokens"].as_array().map(Vec::len)
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
