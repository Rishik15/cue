//! File: mod.rs
//! Purpose: Microphone capture for dictation. One thread owns the cpal stream; the stream opens on demand, lingers briefly after a
//! recording (bursty reuse), then closes so the mic indicator and memory go away. While recording it hands 16 kHz audio to a sink every
//! 200 ms so speech recognition can work during the recording instead of after it.
//! Contents:
//! - init: remember where problems go; start: make the audio thread on the first recording and begin; stop: end it.
//! - Config: per-recording options.
//! - level / live: input level and mic-ready flag for the overlay.
//! - list_microphones: the settings command.
//! - run: the thread loop. Stream setup is in `capture`, resampling in `resample`.

mod capture;
mod resample;

use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use capture::{input_device_names, open_stream, Open, Shared};
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

enum Cmd {
    Start(Config),
    Stop(Done),
}

/// Resamples and forwards audio while a recording runs.
struct Chunker {
    resampler: Resampler,
    sink: ChunkSink,
    on_cap: Arc<dyn Fn() + Send + Sync>,
    capped: bool,
    /// Capacity for the next buffer: the realtime callback should never have to grow it.
    room: usize,
    total: usize,
    next: Instant,
}

impl Chunker {
    /// Sends the audio recorded since the last tick to the sink and arms the next tick; calls `on_cap` once when the recording is full. The replacement buffer is allocated before the lock is taken, so the realtime callback never waits on the allocator.
    fn tick(&mut self, shared: &Shared) {
        let fresh = Vec::with_capacity(self.room);
        let raw = std::mem::replace(&mut *shared.buf(), fresh);
        let out = self.resampler.push(raw);
        self.total += out.len();
        if !out.is_empty() {
            (self.sink)(out);
        }
        self.next = Instant::now() + CHUNK_EVERY;
        let full = shared.recorded.load(Ordering::Relaxed) >= shared.max_samples.load(Ordering::Relaxed);
        if full && !std::mem::replace(&mut self.capped, true) {
            (self.on_cap)();
        }
    }
}

/// Where audio problems go; set by `init`, used when the audio thread is made on the first recording.
static SINK: OnceLock<ErrorSink> = OnceLock::new();
/// Level and mic-ready flags shared with the overlay; made on the first recording and kept, so `live` and `level` read the same flags across thread restarts.
static SHARED: OnceLock<Arc<Shared>> = OnceLock::new();
/// The channel to the audio thread. None until the first recording; replaced when the thread is gone.
static TX: Mutex<Option<Sender<Cmd>>> = Mutex::new(None);

/// Remembers where audio problems go. No thread is made here: the audio thread starts with the first recording (`start`), so a Cue that never records holds no audio thread. Input: the problem sink. Return: Ok always.
pub fn init(on_error: ErrorSink) -> std::io::Result<()> {
    let _ = SINK.set(on_error);
    Ok(())
}

/// Starts the audio thread (512 KB stack, parked between commands). Return: its channel, or the sentence to tell the user when it could not start (reported by the caller after it releases `TX`).
fn spawn_thread() -> Result<Sender<Cmd>, String> {
    let sink = SINK.get().ok_or("The microphone is not set up yet")?.clone();
    let shared = SHARED.get_or_init(Shared::new).clone();
    let (tx, rx) = mpsc::channel();
    crate::guard::spawn("audio", 512 * 1024, move || run(rx, shared, sink))
        .map_err(|e| format!("Cue could not start the microphone: {e}"))?;
    Ok(tx)
}

/// Begins a recording. The audio thread is made on the first call (a thread spawn, no waiting; opening the microphone happens on that thread) and again if it ever died; when it cannot be made the user is told and the recording's `on_fail` runs here, so the caller must show its overlay before calling and not after. Input: the recording's options.
pub fn start(config: Config) {
    let mut slot = TX.lock().unwrap_or_else(|e| e.into_inner());
    let mut cmd = Cmd::Start(config);
    let mut problem = None;
    for _ in 0..2 {
        if let Some(tx) = slot.as_ref() {
            match tx.send(cmd) {
                Ok(()) => return,
                Err(mpsc::SendError(back)) => cmd = back,
            }
        }
        match spawn_thread() {
            Ok(tx) => *slot = Some(tx),
            Err(why) => {
                *slot = None;
                problem = Some(why);
                break;
            }
        }
    }
    drop(slot);
    if let Some(why) = problem {
        if let Some(report) = SINK.get() {
            report(why);
        }
    }
    if let Cmd::Start(config) = cmd {
        (config.on_fail)();
    }
}

/// Ends the recording; `done(tail, total)` runs on the audio thread. If that thread is not there (no recording ever started, or it died) it runs here with nothing, so whoever waits
/// for it (the overlay, the speech session) is never left hanging.
pub fn stop(done: impl FnOnce(Vec<f32>, usize) + Send + 'static) {
    let tx = TX.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let Some(tx) = tx else { return done(Vec::new(), 0) };
    if let Err(mpsc::SendError(Cmd::Stop(done))) = tx.send(Cmd::Stop(Box::new(done))) {
        done(Vec::new(), 0);
    }
}

/// Whether the microphone is delivering audio for the current recording. A cold stream takes about a second to open.
pub fn live() -> bool {
    SHARED.get().is_some_and(|s| s.live.load(Ordering::Relaxed))
}

/// Input level in 0..1 (RMS of the latest callback), for the overlay meter. Return: 0 before the first recording.
pub fn level() -> f32 {
    SHARED.get().map_or(0.0, |s| f32::from_bits(s.level.load(Ordering::Relaxed)))
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
    }
    if open.is_none() {
        match open_stream(shared, on_error, &config.mic) {
            Ok(o) => *open = Some(o),
            Err(e) => on_error(e),
        }
    }
    let o = open.as_ref()?;
    let resampler = Resampler::new(o.rate).map_err(|e| on_error(e)).ok()?;
    shared.max_samples.store(o.rate as usize * config.max_secs as usize, Ordering::Relaxed);
    shared.recorded.store(0, Ordering::Relaxed);
    shared.live.store(false, Ordering::Relaxed);
    let room = o.rate as usize / 2;
    {
        let mut buf = shared.buf();
        buf.clear();
        buf.reserve(room);
    }
    shared.recording.store(true, Ordering::Release);
    Some(Chunker {
        resampler,
        sink: config.sink.clone(),
        on_cap: config.on_cap.clone(),
        capped: false,
        room,
        total: 0,
        next: Instant::now() + CHUNK_EVERY,
    })
}

/// Ends a recording: sends the last samples to `done`. Input: the chunker if one was running, the shared flags and the callback.
fn finish(chunker: Option<Chunker>, shared: &Shared, done: Done) {
    shared.recording.store(false, Ordering::Release);
    shared.live.store(false, Ordering::Relaxed);
    let Some(mut c) = chunker else { return done(Vec::new(), 0) };
    let mut tail = c.resampler.push(std::mem::take(&mut *shared.buf()));
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
                finish(chunker.take(), &shared, done);
                close_at = Some(Instant::now() + linger);
            }
            Err(RecvTimeoutError::Timeout) => match chunker.as_mut() {
                Some(c) => c.tick(&shared),
                None => {
                    open = None;
                    close_at = None;
                    crate::mem::trim_soon();
                }
            },
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}
