//! File: speech/process.rs
//! Purpose: The speech worker as a child process: start it, talk to it over its pipes without ever blocking the caller, stop it.
//! Contents: Worker — the process plus a writer thread (so a busy worker never stalls the speech thread; its queue holds a bounded amount of
//! audio) and a reader thread (replies arrive through a callback; `None` means the pipe closed); Queue — the bound on that audio; Refused — why a request was not queued; spawn — start one (`spawn_command` for any command); send / kill;
//! threads — inference thread count.

use std::io::{BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, OnceLock};

use super::protocol::{read_frame, write_frame, Reply, Request};
use crate::audio::RATE;

/// The most audio waiting in the pipe's writer queue: a worker that reads far slower than real time is stuck, and the queue must not grow without
/// bound. Replaying several long dictations after a crash fits with room to spare.
const MAX_QUEUED: usize = RATE as usize * 300;

/// A worker process with its writer queue; killing it consumes it.
pub struct Worker {
    child: Child,
    tx: Sender<(Request, Vec<f32>)>,
    queued: Arc<Queue>,
    pub serial: u64,
}

/// How much audio waits in the writer queue, with the bound on it.
struct Queue {
    samples: AtomicUsize,
    max: usize,
}

impl Queue {
    /// An empty queue that admits at most `max` samples.
    fn new(max: usize) -> Self {
        Queue { samples: AtomicUsize::new(0), max }
    }

    /// Counts `n` more queued samples. Returns Full, counting nothing, when that would pass the bound.
    fn admit(&self, n: usize) -> Result<(), Refused> {
        if self.samples.load(Ordering::Acquire) + n > self.max {
            return Err(Refused::Full);
        }
        self.samples.fetch_add(n, Ordering::AcqRel);
        Ok(())
    }

    /// Counts `n` samples as written.
    fn written(&self, n: usize) {
        self.samples.fetch_sub(n, Ordering::AcqRel);
    }
}

/// Why a request was not queued.
#[derive(Debug, PartialEq)]
pub enum Refused {
    /// The writer queue is full: the worker is not reading.
    Full,
    /// The writer thread has ended: the worker's stdin is closed or broke, and nothing queued will ever be written.
    Gone,
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

/// The writer thread's loop: sends each queued request until the queue closes or a write fails; a failed write means the worker no longer reads
/// (its stdout may still be open, so the reader would not notice), so `on_broken` tells the session at once. Inputs: the queue, the pipe, the byte counter and the callback.
fn write_requests(rx: &mpsc::Receiver<(Request, Vec<f32>)>, pipe: &mut impl Write, queued: &Queue, on_broken: impl FnOnce()) {
    while let Ok((request, samples)) = rx.recv() {
        let written = write_frame(pipe, &request, &samples);
        queued.written(samples.len());
        if written.is_err() {
            return on_broken();
        }
    }
}

/// Starts a worker process and its two pipe threads. Inputs: its serial and the callback for its replies (`None`: the pipe closed). Returns the worker or why it could not start.
pub fn spawn(serial: u64, on_reply: impl Fn(Option<Reply>) + Send + Sync + 'static) -> Result<Worker, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut command = Command::new(exe);
    command.arg("--speech-worker");
    crate::platform::background_engine(&mut command);
    spawn_command(command, serial, on_reply)
}

/// Starts `command` as a worker with piped stdin and stdout and its two pipe threads (the seam that lets a test run a real child that breaks its pipe). Inputs: the command, the worker's serial and the callback for its replies. Returns the worker or why it could not start.
fn spawn_command(mut command: Command, serial: u64, on_reply: impl Fn(Option<Reply>) + Send + Sync + 'static) -> Result<Worker, String> {
    command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
    let mut child = command.spawn().map_err(|e| format!("Could not start the speech engine: {e}"))?;
    crate::platform::end_with_parent(&child); // a worker stuck in a model load must not outlive Cue holding hundreds of MB
    let (Some(mut stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
        let _ = child.kill(); // never leave a started worker behind
        let _ = child.wait();
        return Err("Could not talk to the speech engine.".into());
    };
    let (tx, rx) = mpsc::channel::<(Request, Vec<f32>)>();
    let queued = Arc::new(Queue::new(MAX_QUEUED));
    let counted = queued.clone();
    let on_reply = Arc::new(on_reply);
    let gone = on_reply.clone();
    let started = thread("speech-tx", move || write_requests(&rx, &mut stdin, &counted, || gone(None))).and_then(|_| {
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
    /// Queues a request for the worker without blocking. Inputs: the request and its audio. Returns Full, sending nothing, when the audio
    /// already waiting is over the limit, and Gone when the writer has ended (it reported that itself; the audio is not left counted).
    pub fn send(&self, request: Request, samples: Vec<f32>) -> Result<(), Refused> {
        let n = samples.len();
        self.queued.admit(n)?;
        if self.tx.send((request, samples)).is_err() {
            self.queued.written(n);
            return Err(Refused::Gone);
        }
        Ok(())
    }

    /// Ends the process and waits for it, so its memory and file handles are released when this returns.
    pub fn kill(mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A write that fails (the worker stopped reading) tells the session once and ends the writer, and the audio it will never write is not left counted.
    #[test]
    fn a_broken_pipe_is_reported_and_stops_the_writer() {
        struct Broken;
        impl Write for Broken {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::ErrorKind::BrokenPipe.into())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let queue = Queue::new(1000);
        let (tx, rx) = mpsc::channel();
        queue.admit(10).unwrap();
        tx.send((Request::End { id: 1 }, vec![0.0; 10])).unwrap();
        let told = std::cell::Cell::new(0);
        write_requests(&rx, &mut Broken, &queue, || told.set(told.get() + 1));
        assert_eq!(told.get(), 1);
        assert!(queue.admit(1000).is_ok(), "the failed request's audio is no longer counted");
    }

    /// PowerShell that closes its own stdin handle (the .NET console cannot) and then waits with stdout still open.
    #[cfg(windows)]
    const CLOSE_STDIN_AND_WAIT: &str = "Add-Type -Name K -Namespace W -MemberDefinition '[DllImport(\"kernel32\")] public static extern bool CloseHandle(System.IntPtr h); [DllImport(\"kernel32\")] public static extern System.IntPtr GetStdHandle(int n);'; [W.K]::CloseHandle([W.K]::GetStdHandle(-10)) | Out-Null; Start-Sleep 30";

    /// A command for a child that closes its stdin at once and keeps its stdout open for a while: the writer's pipe breaks while the reader sees nothing.
    fn child_that_closes_stdin() -> Command {
        #[cfg(unix)]
        let command = {
            let mut c = Command::new("sh");
            c.args(["-c", "exec 0<&-; sleep 30"]);
            c
        };
        #[cfg(windows)]
        let command = {
            let mut c = Command::new("powershell");
            c.args(["-NoProfile", "-Command", CLOSE_STDIN_AND_WAIT]);
            c
        };
        command
    }

    /// A real child whose stdin closes while its stdout stays open: the writer's failure alone tells the session the worker is gone (once), the first Load included, and a send after that is refused as Gone, not Full.
    #[test]
    fn a_real_child_that_closes_stdin_is_reported_gone_and_refuses_as_gone() {
        let (told, heard) = mpsc::channel();
        let worker = spawn_command(child_that_closes_stdin(), 7, move |reply| {
            if reply.is_none() {
                let _ = told.send(());
            }
        })
        .expect("start the child");
        let limit = std::time::Instant::now() + std::time::Duration::from_secs(20);
        let refused = loop {
            if let Err(refused) = worker.send(Request::End { id: 1 }, vec![0.0; 1600]) {
                break refused;
            }
            assert!(std::time::Instant::now() < limit, "the broken pipe was never noticed");
            std::thread::sleep(std::time::Duration::from_millis(5));
        };
        assert_eq!(refused, Refused::Gone);
        assert!(heard.recv_timeout(std::time::Duration::from_secs(5)).is_ok(), "the session is told the worker is gone");
        assert!(heard.try_recv().is_err(), "only once: the reader still sees an open stdout");
        worker.kill();
    }

    /// The writer queue admits audio up to its bound, refuses the piece that would pass it without counting it, and admits again once the writer caught up.
    #[test]
    fn a_full_writer_queue_overloads_without_counting_the_refused_audio() {
        let queue = Queue::new(100);
        assert!(queue.admit(60).is_ok());
        assert!(queue.admit(40).is_ok(), "exactly the bound fits");
        assert_eq!(queue.admit(1), Err(Refused::Full));
        queue.written(60);
        assert!(queue.admit(60).is_ok());
        assert_eq!(queue.admit(1), Err(Refused::Full));
        assert_eq!(MAX_QUEUED, RATE as usize * 300);
    }
}
