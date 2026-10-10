//! Purpose: The speech worker as a child process: start it, talk to it over its pipes without ever blocking the caller, stop it.
//! Contents: Worker — the process plus a writer thread (so a busy worker never stalls the supervisor) and a reader thread
//! (replies arrive through a callback; `None` means the pipe closed); spawn — start one; send / kill; threads — inference thread count.

use std::io::BufReader;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Sender};
use std::sync::OnceLock;

use super::protocol::{read_frame, write_frame, Reply, Request};

pub struct Worker {
    child: Child,
    tx: Sender<(Request, Vec<f32>)>,
    pub serial: u64,
    /// The model finished loading; until then requests wait in the supervisor.
    pub ready: bool,
}

/// Inference threads: half the physical performance cores, 1 to 4. Measured on a 4 P-core laptop: 2 threads decode 30 % faster than 4
/// (the int8 matrix work is memory bound and extra threads only wait on each other).
/// ponytail: one machine measured, scale by a benchmark across CPUs when more hardware is available.
pub fn threads() -> i32 {
    static THREADS: OnceLock<i32> = OnceLock::new();
    *THREADS.get_or_init(|| (crate::platform::performance_cores() / 2).clamp(1, 4) as i32)
}

fn thread(name: &str, body: impl FnOnce() + Send + 'static) -> Result<(), String> {
    std::thread::Builder::new().name(name.into()).stack_size(256 * 1024).spawn(body).map(|_| ()).map_err(|e| e.to_string())
}

pub fn spawn(serial: u64, on_reply: impl Fn(Option<Reply>) + Send + 'static) -> Result<Worker, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut command = Command::new(exe);
    command.arg("--speech-worker").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
    crate::platform::hide_console(&mut command);
    let mut child = command.spawn().map_err(|e| format!("Could not start the speech engine: {e}"))?;
    let (mut stdin, stdout) = (child.stdin.take().ok_or("no pipe")?, child.stdout.take().ok_or("no pipe")?);
    let (tx, rx) = mpsc::channel::<(Request, Vec<f32>)>();
    let started = thread("speech-tx", move || {
        while let Ok((request, samples)) = rx.recv() {
            if write_frame(&mut stdin, &request, &samples).is_err() {
                break; // the worker is gone; the reader reports it
            }
        }
    })
    .and_then(|_| {
        thread("speech-rx", move || {
            let mut out = BufReader::new(stdout);
            while let Ok(Some((reply, _))) = read_frame::<Reply>(&mut out) {
                on_reply(Some(reply));
            }
            on_reply(None);
        })
    });
    if let Err(e) = started {
        let _ = child.kill();
        let _ = child.wait();
        return Err(e);
    }
    Ok(Worker { child, tx, serial, ready: false })
}

impl Worker {
    pub fn send(&self, request: Request, samples: Vec<f32>) {
        let _ = self.tx.send((request, samples));
    }

    /// Ends the process and waits for it, so its memory and file handles are released when this returns.
    pub fn kill(mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
