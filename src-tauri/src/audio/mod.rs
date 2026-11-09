//! File: audio/mod.rs
//! Purpose: Microphone capture for dictation. One thread owns the cpal stream; the stream opens on demand, lingers briefly after a
//! recording (bursty reuse), then closes so the mic indicator and memory go away. While recording it hands 16 kHz audio to a sink every
//! 200 ms so speech recognition can work during the recording instead of after it.
//! Contents:
//! - `recording` is the ring one recording lives in; `Chunker` drains it every 200 ms.
//! - init: remember where problems go; start: make the audio thread on the first recording and begin; stop: end it.
//! - Config: per-recording options.
//! - level / live: input level and mic-ready flag for the overlay.
//! - list_microphones: the settings command.
//! - run: the thread loop. Stream setup is in `capture`, resampling in `resample`.

mod capture;
mod recording;
mod resample;
#[cfg(test)]
mod tests;

use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use crate::slot::Slot;
use capture::{input_device_names, open_stream, Open, Shared};
use recording::Recording;
use resample::Resampler;

pub const RATE: u32 = 16_000;
/// How often finished audio goes to the sink while recording. The only timer that runs, and only during a recording.
const CHUNK_EVERY: Duration = Duration::from_millis(200);

/// Where user-visible audio problems go (the status module); called from the audio thread.
pub type ErrorSink = Arc<dyn Fn(String) + Send + Sync>;
/// Receives 16 kHz mono samples while recording, on the audio thread; must not block.
pub type ChunkSink = Arc<dyn Fn(Vec<f32>) + Send + Sync>;
/// Runs on the audio thread when recording ends, with the last samples and the total number recorded.
type Done = Box<dyn FnOnce(Vec<f32>, usize) + Send>;

/// Options for one recording, read from the user's settings when it starts.
pub struct Config {
    pub mic: Option<String>,
    pub max_secs: u64,
    pub linger: Duration,
    pub sink: ChunkSink,
    /// Runs once on the audio thread when the recording reaches `max_secs`, so the caller can end it instead of losing what comes next.
    pub on_cap: Arc<dyn Fn() + Send + Sync>,
    /// Runs once on the audio thread when the microphone could not be opened (already reported), so the caller can end the recording it started.
    pub on_fail: Arc<dyn Fn() + Send + Sync>,
}

/// What other threads send the audio thread: start a recording or stop it.
enum Cmd {
    Start(Config),
    Stop(Done),
}

/// Resamples and forwards audio while a recording runs.
struct Chunker {
    rec: Arc<Recording>,
    resampler: Resampler,
    sink: ChunkSink,
    on_cap: Arc<dyn Fn() + Send + Sync>,
    on_error: ErrorSink,
    capped: bool,
    overflow_told: bool,
    total: usize,
    next: Instant,
}

impl Chunker {
    /// Sends the audio recorded since the last tick to the sink and arms the next tick; calls `on_cap` once when the recording is full and tells
    /// the user once if the ring ever overflowed (audio was lost, never silently).
    fn tick(&mut self) {
        let drained = self.rec.drain();
        self.note_overflow(drained.overflowed);
        let out = self.resampler.push(drained.samples);
        self.total += out.len();
        if !out.is_empty() {
            (self.sink)(out);
        }
        self.next = Instant::now() + CHUNK_EVERY;
        if self.rec.full() && !std::mem::replace(&mut self.capped, true) {
            (self.on_cap)();
        }
    }

    /// Tells the user once that the ring overflowed and audio was lost. Input: whether the latest drain reported an overflow.
    fn note_overflow(&mut self, overflowed: bool) {
        if overflowed && !std::mem::replace(&mut self.overflow_told, true) {
            (self.on_error)("The microphone audio fell behind and part of the recording was lost.".into());
        }
    }
}

/// Where audio problems go; set by `init`, used when the audio thread is made on the first recording.
static SINK: OnceLock<ErrorSink> = OnceLock::new();
/// Level and mic-ready flags shared with the overlay; made on the first recording and kept, so `live` and `level` read the same flags across thread restarts.
static SHARED: OnceLock<Arc<Shared>> = OnceLock::new();
/// The audio thread's handle: empty until the first recording and again after the thread retired when its stream lingered out.
static TX: Slot<Cmd> = Slot::new();

/// Remembers where audio problems go. No thread is made here: the audio thread starts with the first recording (`start`), so a Cue that never records holds no audio thread. Input: the problem sink. Return: Ok always.
pub fn init(on_error: ErrorSink) -> std::io::Result<()> {
    let _ = SINK.set(on_error);
    Ok(())
}

/// Starts the audio thread (512 KB stack). Input: the receiver it reads. Return: Ok once the thread runs, or why it could not start.
fn spawn_thread(rx: mpsc::Receiver<Cmd>) -> std::io::Result<()> {
    let sink = SINK.get().ok_or_else(|| std::io::Error::other("The microphone is not set up yet"))?.clone();
    let shared = SHARED.get_or_init(Shared::new).clone();
    crate::guard::spawn("audio", 512 * 1024, move || run(rx, shared, sink))
}

/// Begins a recording. Called on the shortcut effects thread (never the keyboard hook thread), so starting the audio thread and waiting for the short `Slot` lock are fine. The audio thread is made on demand (a thread spawn, no waiting; opening the microphone happens on that thread); when it cannot be made the user is told and the recording's `on_fail` runs here, so the caller must show its overlay before calling and not after. Input: the recording's options.
pub fn start(config: Config) {
    if let Err((cmd, why)) = TX.send(Cmd::Start(config), |_, rx| spawn_thread(rx)) {
        if let Some(report) = SINK.get() {
            report(format!("Cue could not start the microphone: {why}"));
        }
        if let Cmd::Start(config) = cmd {
            (config.on_fail)();
        }
    }
}

/// Ends the recording; `done(tail, total)` runs on the audio thread. If that thread is not there (no recording ever started, or it retired or died) it runs here with nothing, so whoever waits
/// for it (the overlay, the speech session) is never left hanging.
pub fn stop(done: impl FnOnce(Vec<f32>, usize) + Send + 'static) {
    if let Err(Cmd::Stop(done)) = TX.try_send(Cmd::Stop(Box::new(done))) {
        done(Vec::new(), 0);
    }
}

/// Whether the microphone is delivering audio for the current recording. A cold stream takes about a second to open.
pub fn live() -> bool {
    SHARED.get().and_then(|s| s.rec()).is_some_and(|r| r.live())
}

/// Input level in 0..1 (RMS of the latest callback), for the overlay meter. Return: 0 before the first recording.
pub fn level() -> f32 {
    SHARED.get().and_then(|s| s.rec()).map_or(0.0, |r| r.level())
}

/// Settings command: the names of the input devices. Return: the names, empty when enumeration fails. It runs on a blocking thread because enumeration can stall on a misbehaving driver, and a plain command would run on the main thread and freeze every window.
#[tauri::command]
pub async fn list_microphones() -> Vec<String> {
    tauri::async_runtime::spawn_blocking(input_device_names).await.unwrap_or_default()
}

/// Prepares a recording: opens the microphone if no stream is open, or opens a fresh one when the stream died or a different microphone was chosen, and builds the chunker. Input: the open stream slot, the shared flags, the problem sink and the recording options. Return: the chunker, or None when the stream or the resampler could not be made (already reported).
fn begin(open: &mut Option<Open>, shared: &Arc<Shared>, on_error: &ErrorSink, config: &Config) -> Option<Chunker> {
    if shared.broken.swap(false, Ordering::Relaxed) || open.as_ref().is_some_and(|o| o.mic != config.mic) {
        *open = None;
        shared.forget();
    }
    if open.is_none() {
        match open_stream(shared, on_error, &config.mic) {
            Ok(o) => *open = Some(o),
            Err(e) => on_error(e),
        }
    }
    let o = open.as_ref()?;
    let resampler = Resampler::new(o.rate).map_err(|e| on_error(e)).ok()?;
    o.rec.start(o.rate as usize * config.max_secs as usize);
    Some(Chunker {
        rec: o.rec.clone(),
        resampler,
        sink: config.sink.clone(),
        on_cap: config.on_cap.clone(),
        on_error: on_error.clone(),
        capped: false,
        overflow_told: false,
        total: 0,
        next: Instant::now() + CHUNK_EVERY,
    })
}

/// Ends a recording: sends the last samples to `done`. Input: the chunker if one was running, and the callback.
fn finish(chunker: Option<Chunker>, done: Done) {
    let Some(mut c) = chunker else { return done(Vec::new(), 0) };
    let drained = c.rec.stop();
    c.note_overflow(drained.overflowed);
    let mut tail = c.resampler.push(drained.samples);
    tail.extend(c.resampler.finish());
    let total = c.total + tail.len();
    done(tail, total);
}

/// The audio thread: blocks until a command, the next chunk time (recording only) or the linger deadline, so nothing polls while idle. When the linger ends it drops the stream, which releases the device. Input: the command channel, the shared flags and the problem sink.
fn run(rx: mpsc::Receiver<Cmd>, shared: Arc<Shared>, on_error: ErrorSink) {
    let mut open: Option<Open> = None;
    let (mut close_at, mut linger) = (None::<Instant>, Duration::ZERO);
    let mut chunker: Option<Chunker> = None;
    loop {
        let wake = [close_at, chunker.as_ref().map(|c| c.next)].into_iter().flatten().min();
        let cmd = match wake {
            Some(t) => rx.recv_timeout(t.saturating_duration_since(Instant::now())),
            None => rx.recv().map_err(|_| RecvTimeoutError::Disconnected),
        };
        let cmd = match cmd {
            Err(RecvTimeoutError::Timeout) if chunker.is_none() => {
                (open, close_at) = (None, None);
                shared.forget();
                crate::mem::trim_soon();
                match TX.retire(&rx) {
                    Some(cmd) => Ok(cmd),
                    None => return,
                }
            }
            other => other,
        };
        match cmd {
            Ok(Cmd::Start(config)) => {
                linger = config.linger;
                chunker = begin(&mut open, &shared, &on_error, &config);
                if chunker.is_none() {
                    (config.on_fail)();
                }
                close_at = None;
            }
            Ok(Cmd::Stop(done)) => {
                finish(chunker.take(), done);
                close_at = Some(Instant::now() + linger);
            }
            Err(RecvTimeoutError::Timeout) => {
                if let Some(c) = chunker.as_mut() {
                    c.tick();
                }
            }
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}
