//! File: speech/process.rs
//! Purpose: The speech worker as a child process: start it, talk to it over its pipes without ever blocking the caller, stop it.
//! Contents: Worker — the process plus a writer thread (so a busy worker never stalls the speech thread; its queue holds a bounded amount of
//! audio) and a reader thread (replies arrive through a callback; `None` means the pipe closed); Overload; spawn — start one; send / kill;
//! threads — inference thread count.

use std::io::BufReader;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, OnceLock};

use super::protocol::{read_frame, write_frame, Reply, Request};
use crate::audio::RATE;

/// The most audio waiting in the pipe's writer queue: a worker that reads far slower than real time is stuck, and the queue must not grow without
/// bound. Replaying several long dictations after a crash fits with room to spare.
const MAX_QUEUED: usize = RATE as usize * 300;

pub struct Worker {
    child: Child,
    tx: Sender<(Request, Vec<f32>)>,
    queued: Arc<AtomicUsize>,
    pub serial: u64,
}

/// The writer queue is full: the worker is not reading.
#[derive(Debug)]
pub struct Overload;

/// Inference threads: half the physical performance cores, 2 to 4; under 4 logical processors (a small PC or a two-processor virtual machine) 2, and 1 only on a single processor.
/// Measured on a 4 P-core laptop: 2 threads decode 30 % faster than 4 (the int8 matrix work is memory bound and extra threads only wait
/// on each other). One thread decodes about twice as slowly as two, and the engine runs below normal priority, so on a small PC it cannot starve the keyboard hook or the window.
/// shortcut: one machine measured, scale by a benchmark across CPUs when more hardware is available.
pub fn threads() -> i32 {
    static THREADS: OnceLock<i32> = OnceLock::new();
    *THREADS.get_or_init(|| {
        let logical = std::thread::available_parallelism().map_or(1, |n| n.get());
        count(logical, crate::platform::performance_cores())
    })
}

/// The thread count for the given processor counts. Inputs: logical processors and performance cores. Returns the inference thread count.
fn count(logical: usize, performance: usize) -> i32 {
    if logical < 4 {
        return logical.min(2) as i32;
    }
    (performance / 2).clamp(2, 4) as i32
}

/// Starts one pipe thread. Inputs: its name and body. Returns why it could not start.
fn thread(name: &str, body: impl FnOnce() + Send + 'static) -> Result<(), String> {
    crate::guard::spawn(name, 256 * 1024, body).map(|_| ()).map_err(|e| e.to_string())
}

/// Starts a worker process and its two pipe threads. Inputs: its serial and the callback for its replies (`None`: the pipe closed). Returns the worker or why it could not start.
pub fn spawn(serial: u64, on_reply: impl Fn(Option<Reply>) + Send + 'static) -> Result<Worker, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut command = Command::new(exe);
    command.arg("--speech-worker").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
    crate::platform::background_engine(&mut command);
    let mut child = command.spawn().map_err(|e| format!("Could not start the speech engine: {e}"))?;
    crate::platform::end_with_parent(&child); // a worker stuck in a model load must not outlive Cue holding hundreds of MB
    let (Some(mut stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
        let _ = child.kill(); // never leave a started worker behind
        let _ = child.wait();
        return Err("Could not talk to the speech engine.".into());
    };
    let (tx, rx) = mpsc::channel::<(Request, Vec<f32>)>();
    let queued = Arc::new(AtomicUsize::new(0));
    let counted = queued.clone();
    let started = thread("speech-tx", move || {
        while let Ok((request, samples)) = rx.recv() {
            let written = write_frame(&mut stdin, &request, &samples);
            counted.fetch_sub(samples.len(), Ordering::AcqRel);
            if written.is_err() {
                break;
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
    Ok(Worker { child, tx, queued, serial })
}

impl Worker {
    /// Queues a request for the worker without blocking. Inputs: the request and its audio. Returns Overload, sending nothing, when the audio
    /// already waiting is over the limit.
    pub fn send(&self, request: Request, samples: Vec<f32>) -> Result<(), Overload> {
        if self.queued.load(Ordering::Acquire) + samples.len() > MAX_QUEUED {
            return Err(Overload);
        }
        self.queued.fetch_add(samples.len(), Ordering::AcqRel);
        let _ = self.tx.send((request, samples));
        Ok(())
    }

    /// Ends the process and waits for it, so its memory and file handles are released when this returns.
    pub fn kill(mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
