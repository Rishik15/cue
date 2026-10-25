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

fn count(logical: usize, performance: usize) -> i32 {
    if logical < 4 {
        return logical.min(2) as i32;
    }
    (performance / 2).clamp(2, 4) as i32
}

fn thread(name: &str, body: impl FnOnce() + Send + 'static) -> Result<(), String> {
    crate::guard::spawn(name, 256 * 1024, body).map(|_| ()).map_err(|e| e.to_string())
}

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

#[cfg(test)]
mod tests {
    use super::count;

    #[test]
    fn small_pcs_get_two_threads_and_one_processor_gets_one() {
        assert_eq!(count(1, 1), 1);
        assert_eq!(count(2, 2), 2);
        assert_eq!(count(3, 3), 2);
        assert_eq!(count(4, 2), 2); // half of two fast cores is one: the floor is two
        assert_eq!(count(16, 8), 4);
    }
}
