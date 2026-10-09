//! Purpose: Microphone capture for dictation. One thread owns the cpal stream; the stream opens on demand,
//! lingers briefly after a recording (bursty reuse), then closes so the mic indicator and memory go away.
//! Contents: init / start / stop — commands to the audio thread; Config — per-recording options; level — live input
//! level for the overlay; list_microphones — settings command; run — the thread loop. Stream setup is in `capture`, resampling in `resample`.

mod capture;
mod resample;

use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use capture::{input_device_names, open_stream, Open, Shared};
pub use resample::to_16k;

pub const RATE: u32 = 16_000;

/// Where user-visible audio problems go (the status module); called from the audio thread.
pub type ErrorSink = Arc<dyn Fn(String) + Send + Sync>;
type Done = Box<dyn FnOnce(Vec<f32>) + Send>;

/// Options for one recording, read from the user's settings when it starts.
pub struct Config {
    pub mic: Option<String>,
    pub max_secs: u64,
    pub linger: Duration,
}

enum Cmd {
    Start(Config),
    Stop(Done),
}

static TX: OnceLock<Sender<Cmd>> = OnceLock::new();
static SHARED: OnceLock<Arc<Shared>> = OnceLock::new();

pub fn init(on_error: ErrorSink) -> std::io::Result<()> {
    let shared = Shared::new();
    let (tx, rx) = mpsc::channel();
    let s = shared.clone();
    std::thread::Builder::new().name("audio".into()).stack_size(512 * 1024).spawn(move || run(rx, s, on_error))?;
    let _ = (TX.set(tx), SHARED.set(shared));
    Ok(())
}

pub fn start(config: Config) {
    if let Some(tx) = TX.get() {
        let _ = tx.send(Cmd::Start(config));
    }
}

/// Ends the recording; `done` runs on the audio thread with 16 kHz mono samples.
pub fn stop(done: impl FnOnce(Vec<f32>) + Send + 'static) {
    if let Some(tx) = TX.get() {
        let _ = tx.send(Cmd::Stop(Box::new(done)));
    }
}

/// Input level in 0..1 (RMS of the latest callback), for the overlay meter.
pub fn level() -> f32 {
    SHARED.get().map_or(0.0, |s| f32::from_bits(s.level.load(Ordering::Relaxed)))
}

#[tauri::command]
pub fn list_microphones() -> Vec<String> {
    input_device_names()
}

fn begin(open: &mut Option<Open>, shared: &Arc<Shared>, on_error: &ErrorSink, config: &Config) {
    if open.as_ref().is_some_and(|o| o.mic != config.mic) {
        *open = None; // a different microphone was chosen
    }
    if open.is_none() {
        match open_stream(shared, on_error, &config.mic) {
            Ok(o) => *open = Some(o),
            Err(e) => on_error(e),
        }
    }
    if let Some(o) = open {
        shared.max_samples.store(o.rate as usize * config.max_secs as usize, Ordering::Relaxed);
        let mut buf = shared.buf.lock().unwrap();
        buf.clear();
        buf.reserve(o.rate as usize * 10);
        shared.recording.store(true, Ordering::Release);
    }
}

fn run(rx: mpsc::Receiver<Cmd>, shared: Arc<Shared>, on_error: ErrorSink) {
    let mut open: Option<Open> = None;
    let (mut close_at, mut linger) = (None::<Instant>, Duration::ZERO);
    loop {
        // Blocks until a command, or until the linger deadline when the stream is idle. No polling.
        let cmd = match close_at {
            Some(t) => rx.recv_timeout(t.saturating_duration_since(Instant::now())),
            None => rx.recv().map_err(|_| RecvTimeoutError::Disconnected),
        };
        match cmd {
            Ok(Cmd::Start(config)) => {
                linger = config.linger;
                begin(&mut open, &shared, &on_error, &config);
                close_at = None;
            }
            Ok(Cmd::Stop(done)) => {
                shared.recording.store(false, Ordering::Release);
                let raw = std::mem::take(&mut *shared.buf.lock().unwrap());
                let rate = open.as_ref().map_or(RATE, |o| o.rate);
                close_at = Some(Instant::now() + linger);
                done(to_16k(&raw, rate));
            }
            Err(RecvTimeoutError::Timeout) => {
                open = None; // dropping the stream releases the device
                close_at = None;
                crate::mem::trim_soon();
            }
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}
