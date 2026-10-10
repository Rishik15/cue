//! Purpose: Microphone capture for dictation. One thread owns the cpal stream; the stream opens on demand, lingers briefly after a
//! recording (bursty reuse), then closes so the mic indicator and memory go away. While recording it hands 16 kHz audio to a sink every
//! 200 ms so speech recognition can work during the recording instead of after it.
//! Contents: init / start / stop — commands to the audio thread; Config — per-recording options; level / live — input level and
//! mic-ready flag for the overlay; list_microphones — settings command; run — the thread loop. Stream setup is in `capture`, resampling in `resample`.

mod capture;
mod resample;

use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, OnceLock};
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
}

enum Cmd {
    Start(Config),
    Stop(Done),
}

/// Resamples and forwards audio while a recording runs.
struct Chunker {
    resampler: Resampler,
    sink: ChunkSink,
    /// Capacity for the next buffer: the realtime callback should never have to grow it.
    room: usize,
    total: usize,
    next: Instant,
}

impl Chunker {
    fn tick(&mut self, shared: &Shared) {
        let raw = std::mem::replace(&mut *shared.buf.lock().unwrap(), Vec::with_capacity(self.room));
        let out = self.resampler.push(&raw);
        self.total += out.len();
        if !out.is_empty() {
            (self.sink)(out);
        }
        self.next = Instant::now() + CHUNK_EVERY;
    }
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

/// Ends the recording; `done(tail, total)` runs on the audio thread.
pub fn stop(done: impl FnOnce(Vec<f32>, usize) + Send + 'static) {
    if let Some(tx) = TX.get() {
        let _ = tx.send(Cmd::Stop(Box::new(done)));
    }
}

/// Whether the microphone is delivering audio for the current recording. A cold stream takes about a second to open.
pub fn live() -> bool {
    SHARED.get().is_some_and(|s| s.live.load(Ordering::Relaxed))
}

/// Input level in 0..1 (RMS of the latest callback), for the overlay meter.
pub fn level() -> f32 {
    SHARED.get().map_or(0.0, |s| f32::from_bits(s.level.load(Ordering::Relaxed)))
}

#[tauri::command]
pub fn list_microphones() -> Vec<String> {
    input_device_names()
}

fn begin(open: &mut Option<Open>, shared: &Arc<Shared>, on_error: &ErrorSink, config: &Config) -> Option<Chunker> {
    if shared.broken.swap(false, Ordering::Relaxed) || open.as_ref().is_some_and(|o| o.mic != config.mic) {
        *open = None; // the stream died, or a different microphone was chosen: open a fresh one
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
        let mut buf = shared.buf.lock().unwrap();
        buf.clear();
        buf.reserve(room);
    }
    shared.recording.store(true, Ordering::Release);
    Some(Chunker { resampler, sink: config.sink.clone(), room, total: 0, next: Instant::now() + CHUNK_EVERY })
}

fn finish(chunker: Option<Chunker>, shared: &Shared, done: Done) {
    shared.recording.store(false, Ordering::Release);
    shared.live.store(false, Ordering::Relaxed);
    let Some(mut c) = chunker else { return done(Vec::new(), 0) };
    let mut tail = c.resampler.push(&std::mem::take(&mut *shared.buf.lock().unwrap()));
    tail.extend(c.resampler.finish());
    let total = c.total + tail.len();
    done(tail, total);
}

fn run(rx: mpsc::Receiver<Cmd>, shared: Arc<Shared>, on_error: ErrorSink) {
    let mut open: Option<Open> = None;
    let (mut close_at, mut linger) = (None::<Instant>, Duration::ZERO);
    let mut chunker: Option<Chunker> = None;
    loop {
        // Blocks until a command, the next chunk time (recording only), or the linger deadline. No polling while idle.
        let wake = [close_at, chunker.as_ref().map(|c| c.next)].into_iter().flatten().min();
        let cmd = match wake {
            Some(t) => rx.recv_timeout(t.saturating_duration_since(Instant::now())),
            None => rx.recv().map_err(|_| RecvTimeoutError::Disconnected),
        };
        match cmd {
            Ok(Cmd::Start(config)) => {
                linger = config.linger;
                chunker = begin(&mut open, &shared, &on_error, &config);
                close_at = None;
            }
            Ok(Cmd::Stop(done)) => {
                finish(chunker.take(), &shared, done);
                close_at = Some(Instant::now() + linger);
            }
            Err(RecvTimeoutError::Timeout) => match chunker.as_mut() {
                Some(c) => c.tick(&shared),
                None => {
                    open = None; // dropping the stream releases the device
                    close_at = None;
                    crate::mem::trim_soon();
                }
            },
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}
