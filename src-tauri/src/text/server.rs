//! Purpose: The llama.cpp server as a child process: start it when the first request needs it, wait until it answers, stop it after the idle time.
//! Contents: Conn — where and how to talk to the server (port and API key); connect — a connection to a server running the given model (starting it, or restarting it when another model was loaded); used — a request finished (starts the idle clock, or stops at once when idle time is 0); stop — end the process; spawn / wait_ready — start and wait; watch — the idle timer thread.
//! Security: the server listens on 127.0.0.1 only, demands a random per-launch API key (passed in the environment, not on the command line) and has its `/slots` endpoint (which shows recent prompts) turned off, so another program or a web page cannot read or use it.
//! Cost: nothing while stopped. The process is tied to Cue (`platform::end_with_parent`), so it ends when Cue does, however Cue ends.

use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
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

/// Longest wait for the model to load. Loading 1.1 GB from a slow disk takes seconds; a minute and a half means something is wrong.
const READY_LIMIT: Duration = Duration::from_secs(90);

fn idle_secs(app: &AppHandle) -> i64 {
    crate::settings::get_i64(app, "unload_after_secs", 120)
}

/// A connection to a server running `model`, starting it first if none is running, it has died, or it holds another model.
pub fn connect(app: &AppHandle, model: &'static TextModel) -> Result<Conn, String> {
    let mut running = RUNNING.lock().unwrap();
    if let Some(r) = running.as_mut() {
        if r.model == model.id && matches!(r.child.try_wait(), Ok(None)) {
            return Ok(r.conn.clone());
        }
    }
    if let Some(mut old) = running.take() {
        let _ = old.child.kill();
        let _ = old.child.wait();
    }
    let started = spawn(app, model)?;
    let conn = started.conn.clone();
    *running = Some(started);
    *LAST_USED.lock().unwrap() = Some(Instant::now());
    watch(app);
    Ok(conn)
}

/// A request finished: the idle clock restarts, or the server stops now when the user chose "Immediately".
pub fn used(app: &AppHandle) {
    *LAST_USED.lock().unwrap() = Some(Instant::now());
    if idle_secs(app) == 0 {
        stop();
    }
}

/// Ends the process and waits for it, so the model file is released when this returns (Windows will not delete an open file).
pub fn stop() {
    if let Some(mut r) = RUNNING.lock().unwrap().take() {
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
    let child = command.spawn().map_err(|e| format!("Could not start the text engine: {e}"))?;
    crate::platform::end_with_parent(&child);
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
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(150));
    }
    Err("The text engine took too long to start.".into())
}

/// Sleeps until the idle time has passed since the last request, then stops the server. It waits on a deadline, not a poll.
fn watch(app: &AppHandle) {
    if idle_secs(app) <= 0 {
        return; // never unload, or unloaded by `used` right after each request
    }
    let app = app.clone();
    let _ = std::thread::Builder::new().name("text-idle".into()).stack_size(256 * 1024).spawn(move || loop {
        let idle = Duration::from_secs(idle_secs(&app).max(1) as u64);
        let since = LAST_USED.lock().unwrap().map_or(idle, |t| t.elapsed());
        if since >= idle {
            return stop();
        }
        std::thread::sleep(idle - since);
    });
}
