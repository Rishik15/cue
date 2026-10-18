//! Purpose: The llama.cpp server as a child process: start it when the first request needs it, wait until it answers, stop it after the idle time and never while a request holds it.
//! Contents: Conn — where and how to talk to the server (port and API key); Lease — a request's hold on the server (its drop restarts the idle clock, or stops at once when idle time is 0 and nothing else holds it); connect — a Lease on a server running the given model (starting it, or restarting it when another model was loaded); stop — end the process unconditionally (model delete); stop_idle — end it unless held; spawn / wait_ready — start and wait; watch — the idle timer thread.
//! Security: the server listens on 127.0.0.1 only, demands a random per-launch API key (passed in the environment, not on the command line) and has its `/slots` endpoint (which shows recent prompts) turned off, so another program or a web page cannot read or use it.
//! Cost: nothing while stopped. The process is tied to Cue (`platform::end_with_parent`), so it ends when Cue does, however Cue ends.

use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::AppHandle;

use super::catalog::{self, TextModel};

struct Running {
    child: Child,
    conn: Conn,
    model: &'static str,
}

/// How to reach the running server.
#[derive(Clone)]
pub struct Conn {
    pub port: u16,
    pub key: String,
}

/// 24 random bytes from the OS as hex. A new key for every server start, so a stale one is worthless.
fn new_key() -> Result<String, String> {
    let mut bytes = [0u8; 24];
    getrandom::fill(&mut bytes).map_err(|e| format!("No random numbers for the text engine: {e}"))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

static RUNNING: Mutex<Option<Running>> = Mutex::new(None);
static LAST_USED: Mutex<Option<Instant>> = Mutex::new(None);
/// One idle watcher at a time: each server start used to add another thread.
static WATCHING: AtomicBool = AtomicBool::new(false);

/// Requests that hold the server right now (see `Lease`). The idle watcher and "Immediately" stop it only at 0, so a request is never killed mid-answer.
static IN_USE: AtomicUsize = AtomicUsize::new(0);

/// Longest wait for the model to load. Loading 1.1 GB from a slow disk takes seconds; a minute and a half means something is wrong.
const READY_LIMIT: Duration = Duration::from_secs(90);

fn idle_secs(app: &AppHandle) -> i64 {
    crate::settings::get_i64(app, "unload_after_secs", 120)
}

/// A request's hold on the running server. While it lives the server is not stopped by the idle timer; dropping it restarts the idle clock,
/// or stops the server at once when the user chose "Immediately" and no other request holds it.
pub struct Lease {
    pub conn: Conn,
    app: AppHandle,
}

impl Drop for Lease {
    fn drop(&mut self) {
        release(&self.app);
    }
}

fn release(app: &AppHandle) {
    // Stamp before the count drops, so a watcher that sees 0 also sees the fresh stamp.
    *LAST_USED.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now());
    IN_USE.fetch_sub(1, Ordering::AcqRel);
    if idle_secs(app) == 0 {
        stop_idle();
    }
}

/// A lease on a server running `model`, starting it first if none is running, it has died, or it holds another model.
pub fn connect(app: &AppHandle, model: &'static TextModel) -> Result<Lease, String> {
    // Counted before anything else, so no stop between here and the return can take the server from this request.
    IN_USE.fetch_add(1, Ordering::AcqRel);
    match start(app, model) {
        Ok(conn) => Ok(Lease { conn, app: app.clone() }),
        Err(e) => {
            release(app);
            Err(e)
        }
    }
}

fn start(app: &AppHandle, model: &'static TextModel) -> Result<Conn, String> {
    // The lock is held only to look at and swap the entry, never while the server starts (up to 90 s): `stop` must not wait behind a slow start.
    let old = {
        let mut running = RUNNING.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(r) = running.as_mut() {
            // The owner is checked again on reuse: the engine may have died and another program taken its port since the last request.
            if r.model == model.id
                && matches!(r.child.try_wait(), Ok(None))
                && crate::platform::port_owner(r.conn.port) == Some(r.child.id())
            {
                return Ok(r.conn.clone());
            }
        }
        running.take()
    };
    if let Some(mut old) = old {
        let _ = old.child.kill();
        let _ = old.child.wait();
    }
    let started = spawn(app, model)?;
    let conn = started.conn.clone();
    *RUNNING.lock().unwrap_or_else(|e| e.into_inner()) = Some(started);
    watch(app);
    Ok(conn)
}

/// Whether the server stays loaded between requests (idle time above 0). Warming it ahead of a request is pointless otherwise.
pub fn keeps_warm(app: &AppHandle) -> bool {
    idle_secs(app) != 0
}

/// Ends the process and waits for it, so the model file is released when this returns (Windows will not delete an open file).
/// Unconditional: the caller (a model delete) must hold the request lock so no request is in flight.
pub fn stop() {
    kill(RUNNING.lock().unwrap_or_else(|e| e.into_inner()).take());
}

/// Stops the server unless a request holds it. The count is checked under the `RUNNING` lock, which `start` takes after counting, so the two cannot cross.
/// Returns false when a request still holds it.
fn stop_idle() -> bool {
    let taken = {
        let mut running = RUNNING.lock().unwrap_or_else(|e| e.into_inner());
        if IN_USE.load(Ordering::Acquire) > 0 {
            return false;
        }
        running.take()
    };
    kill(taken);
    true
}

fn kill(running: Option<Running>) {
    if let Some(mut r) = running {
        let _ = r.child.kill();
        let _ = r.child.wait();
    }
}

fn spawn(app: &AppHandle, model: &'static TextModel) -> Result<Running, String> {
    let (Some(exe), Some(path)) = (catalog::server_path(app), catalog::model_path(app, model)) else {
        return Err("Cue has no folder to store models in.".into());
    };
    // Ask the OS for a free port, then hand it over. Another program could grab it in between; the health check then fails and says so.
    let port =
        TcpListener::bind("127.0.0.1:0").and_then(|l| l.local_addr()).map_err(|e| format!("No free port for the text engine: {e}"))?.port();
    let key = new_key()?;
    let mut command = Command::new(exe);
    command
        .args([
            "--host",
            "127.0.0.1",
            "--port",
            &port.to_string(),
            "-c",
            "4096",
            "-np",
            "1",
            "-ngl",
            "99",
            "--no-webui",
            "--no-slots",
            "-t",
        ])
        .arg(crate::speech::process::threads().to_string())
        .arg("-m")
        .arg(path)
        .env("LLAMA_API_KEY", &key)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    crate::platform::hide_console(&mut command);
    let mut child = command.spawn().map_err(|e| format!("Could not start the text engine: {e}"))?;
    if !crate::platform::end_with_parent(&child) {
        // Unbound, it would keep running (and hold the model in memory) if Cue crashed.
        let _ = child.kill();
        let _ = child.wait();
        return Err("Windows would not tie the text engine to Cue, so it was stopped.".into());
    }
    let mut running = Running { child, conn: Conn { port, key }, model: model.id };
    match wait_ready(&mut running) {
        Ok(()) => Ok(running),
        Err(e) => {
            let _ = running.child.kill();
            let _ = running.child.wait();
            Err(e)
        }
    }
}

/// Polls the server's health endpoint (200 once the model is loaded, 503 while it loads). Bounded, and only while starting.
fn wait_ready(r: &mut Running) -> Result<(), String> {
    let url = format!("http://127.0.0.1:{}/health", r.conn.port);
    let started = Instant::now();
    while started.elapsed() < READY_LIMIT {
        if !matches!(r.child.try_wait(), Ok(None)) {
            return Err("The text engine stopped while starting. Delete the text model and download it again.".into());
        }
        if ureq::get(&url).timeout(Duration::from_secs(2)).call().is_ok() {
            // Between choosing the port and the server binding it, another program could have taken it; it would then be sent the key and the user's text.
            let owner = crate::platform::port_owner(r.conn.port);
            return if owner == Some(r.child.id()) {
                Ok(())
            } else {
                crate::log::note(&format!("text engine port {} is held by process {owner:?}, expected {}", r.conn.port, r.child.id()));
                Err("Another program is using the text engine's port, so it was stopped.".into())
            };
        }
        std::thread::sleep(Duration::from_millis(150));
    }
    Err("The text engine took too long to start.".into())
}

/// Sleeps until the idle time has passed since the last request, then stops the server. It waits on a deadline, not a poll.
fn watch(app: &AppHandle) {
    if idle_secs(app) <= 0 || WATCHING.swap(true, Ordering::AcqRel) {
        return; // never unload, or unloaded by `used` right after each request, or a watcher is already running
    }
    let app = app.clone();
    let spawned = crate::guard::spawn("text-idle", 256 * 1024, move || {
        let _done = crate::guard::Release(&WATCHING);
        loop {
            let secs = idle_secs(&app);
            if secs <= 0 {
                return; // the setting changed to "never" or "immediately" meanwhile
            }
            let idle = Duration::from_secs(secs as u64);
            let since = LAST_USED.lock().unwrap_or_else(|e| e.into_inner()).map_or(idle, |t| t.elapsed());
            if since >= idle && stop_idle() {
                return;
            }
            // A request still holds it: check again after a full idle time (its end stamps LAST_USED, so the clock restarts then).
            std::thread::sleep(if since >= idle { idle } else { idle - since });
        }
    });
    if spawned.is_err() {
        WATCHING.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_held_server_is_not_stopped() {
        IN_USE.store(1, Ordering::Release);
        assert!(!stop_idle());
        IN_USE.store(0, Ordering::Release);
        assert!(stop_idle());
    }
}
