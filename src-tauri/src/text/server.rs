//! File: text/server.rs
//! Purpose: The real llama.cpp server behind the lifecycle engine: start the process, ask whether it has loaded, end it. Also the one process-wide engine and the calls the rest of the text model makes on it.
//! Contents: Sidecar — the `Host` that starts llama.cpp; Server — one running process (a `Child`); connect — a lease on a server running the model; stop_for_delete — refuse calls and stop for a model delete; keeps_warm — whether the server stays loaded between requests; Lease — the engine's lease type.
//! Security: the server listens on 127.0.0.1 only, demands a random per-launch API key (passed in the environment, not on the command line) and has its `/slots` endpoint (which shows recent prompts) turned off, so another program or a web page cannot read or use it.
//! Cost: nothing while stopped. The process is tied to Cue (`platform::end_with_parent`), so it ends when Cue does, however Cue ends.

use std::net::TcpListener;
use std::process::{Command, Stdio};
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use super::cancel::Cancel;
use super::catalog::{self, TextModel};
use super::engine::{self, Child, DeleteGuard, Engine, Fail, Host, Probe, SystemClock};

pub use super::engine::Conn;

/// A request's hold on the running server.
pub type Lease = engine::Lease<Sidecar, SystemClock>;
/// The handle that can end the server of one lease from another thread.
pub type Aborter = engine::Aborter<Sidecar, SystemClock>;

static ENGINE: LazyLock<Arc<Engine<Sidecar, SystemClock>>> = LazyLock::new(|| Engine::new(Sidecar, SystemClock::new()));

/// Longest wait for one answer from the health endpoint. Short, so a cancel is noticed within a second.
const HEALTH_TIMEOUT: Duration = Duration::from_millis(500);

/// A lease on a server running `model`, starting it first if needed. Inputs: the model and the cancel token of the caller. Returns the lease, `Cancelled`, or the reason the server could not start.
pub fn connect(model: &'static TextModel, cancel: &Cancel) -> Result<Lease, Fail> {
    ENGINE.acquire(model.id, cancel)
}

/// Refuses new calls, waits for the running one and stops the server so the model files can be deleted. Returns the guard that lets calls through again when dropped.
pub fn stop_for_delete() -> DeleteGuard<Sidecar, SystemClock> {
    ENGINE.stop_for_delete()
}

/// Whether the server stays loaded between requests (idle time above 0). Warming it ahead of a request is pointless otherwise.
pub fn keeps_warm() -> bool {
    ENGINE.keeps_warm()
}

/// Starts llama.cpp processes and reads the settings and threads of the real app.
pub struct Sidecar;

/// One llama.cpp process and how to reach it.
pub struct Server {
    child: std::process::Child,
    conn: Conn,
}

impl Host for Sidecar {
    type Child = Server;

    /// Starts the server on the GPU, or on the CPU alone when `fallback` is set or the GPU was found broken earlier. Does not wait for the model to load.
    fn launch(&self, model: &'static str, fallback: bool) -> Result<Server, String> {
        let gpu = !fallback && !crate::settings::get_bool("text_gpu_off", false);
        let model = catalog::find(model).ok_or("The text model is not in the catalogue.")?;
        let app = super::APP.get().ok_or("The text engine is not ready yet.")?;
        let (Some(exe), Some(path)) = (catalog::server_path(app), catalog::model_path(app, model)) else {
            return Err("Cue has no folder to store models in.".into());
        };
        let port = TcpListener::bind("127.0.0.1:0")
            .and_then(|l| l.local_addr())
            .map_err(|e| format!("No free port for the text engine: {e}"))?
            .port();
        let key = new_key()?;
        let mut command = server_command(&exe, &path, port, &key, (gpu, model.context));
        crate::platform::background_engine(&mut command);
        let mut child = command.spawn().map_err(|e| format!("Could not start the text engine: {e}"))?;
        if !crate::platform::end_with_parent(&child) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Windows would not tie the text engine to Cue, so it was stopped.".into());
        }
        Ok(Server { child, conn: Conn { port, key } })
    }

    /// Seconds the user chose to keep the model loaded after the last request.
    fn idle_secs(&self) -> i64 {
        crate::settings::get_i64("unload_after_secs", 120)
    }

    /// A failed GPU start is worth a CPU start; a CPU start that failed is not.
    fn retry_after_failure(&self) -> bool {
        !crate::settings::get_bool("text_gpu_off", false)
    }

    /// Remembers the GPU is broken so later starts skip it. shortcut: never retries the GPU after a driver update; "Remove All Cue Data" resets it.
    fn fallback_worked(&self) {
        crate::settings::remember("text_gpu_off", true.into());
    }

    /// Starts a guarded thread for the idle watcher.
    fn spawn(&self, body: Box<dyn FnOnce() + Send>) -> bool {
        crate::guard::spawn("text-idle", 256 * 1024, body).is_ok()
    }

    /// Appends to the support log.
    fn note(&self, line: &str) {
        crate::log::note(line);
    }
}

impl Child for Server {
    /// The process still runs and owns its port; the engine may have died and another program taken the port since the last request.
    fn alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None)) && crate::platform::port_owner(self.conn.port) == Some(self.child.id())
    }

    /// Asks the health endpoint (200 once the model is loaded, 503 while it loads), and checks the port owner before the key is ever sent.
    fn probe(&mut self) -> Probe {
        if let Ok(Some(status)) = self.child.try_wait() {
            crate::log::note(&format!("text engine exited while starting: {status}"));
            return Probe::Failed(exit_message(status.code()));
        }
        let url = format!("http://127.0.0.1:{}/health", self.conn.port);
        if ureq::get(&url).config().timeout_global(Some(HEALTH_TIMEOUT)).build().call().is_err() {
            return Probe::Loading;
        }
        let owner = crate::platform::port_owner(self.conn.port);
        if owner == Some(self.child.id()) {
            return Probe::Ready(self.conn.clone());
        }
        crate::log::note(&format!("text engine port {} is held by process {owner:?}, expected {}", self.conn.port, self.child.id()));
        Probe::Failed("Another program is using the text engine's port, so it was stopped.".into())
    }

    /// Ends the process and waits for it, so the model file is released when this returns (Windows will not delete an open file).
    fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// 24 random bytes from the OS as hex. A new key for every server start, so a stale one is worthless.
fn new_key() -> Result<String, String> {
    let mut bytes = [0u8; 24];
    getrandom::fill(&mut bytes).map_err(|e| format!("No random numbers for the text engine: {e}"))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// The command line of the server. Inputs: the server program, the model file, the port, the API key, and whether to use the GPU with the context size in tokens. Returns the command, not yet started.
fn server_command(exe: &std::path::Path, model: &std::path::Path, port: u16, key: &str, (gpu, context): (bool, u32)) -> Command {
    let mut command = Command::new(exe);
    command
        .args(["--host", "127.0.0.1", "--port", &port.to_string(), "-c", &context.to_string(), "-np", "1", "-ngl", if gpu { "99" } else { "0" }])
        .args(["--no-webui", "--no-slots", "-t"])
        .arg(crate::speech::process::threads().to_string())
        .arg("-m")
        .arg(model)
        .env("LLAMA_API_KEY", key)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}

/// What to tell the user when the engine exits at start. Windows refuses to load unsigned DLLs under Smart App Control (0xC0E90002) or
/// a failed signature check (0xC0000428); re-downloading the model cannot help there.
fn exit_message(code: Option<i32>) -> String {
    match code.map(|c| c as u32) {
        Some(0xC0E9_0002 | 0xC000_0428) => "Windows Smart App Control blocked Cue's text engine. Turn Smart App Control off in Windows Security > App & browser control to use AI actions; everything else in Cue works without it.".into(),
        _ => "The text engine stopped while starting. Delete the text model and download it again.".into(),
    }
}
