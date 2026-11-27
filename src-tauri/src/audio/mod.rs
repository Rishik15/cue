//! File: audio/mod.rs
//! Purpose: Microphone capture for dictation. One thread owns the cpal stream; the stream opens on demand, lingers briefly after a
//! recording (bursty reuse), then closes so the mic indicator and memory go away. While recording it hands 16 kHz audio to a sink every
//! 200 ms so speech recognition can work during the recording instead of after it.
//! Contents:
//! - `recording` is the ring one recording lives in; `Chunker` drains it every 200 ms.
//! - init: remember where problems go; start: make the audio thread on the first recording and begin; stop: end it; after: run a job on the audio thread after a delay (the thread stays up while one is pending).
//! - Config: per-recording options; End and Loss: why a recording ended and what it lost; Heard: what the last drain returns.
//! - level / live: input level and mic-ready flag for the overlay.
//! - list_microphones: the settings command.
//! - run: the thread loop. Stream setup is in `capture`, resampling in `resample`.

mod capture;
mod recording;
mod resample;
#[cfg(test)]
mod tests;

use std::cell::Cell;
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
/// How long past the recording limit the clock ends a recording whose stream delivers nothing; the sample limit ends a healthy one first.
const DEADLINE_SLACK: Duration = Duration::from_secs(2);

/// Where user-visible audio problems go (the status module); called from the audio thread.
pub type ErrorSink = Arc<dyn Fn(String) + Send + Sync>;
/// Receives 16 kHz mono samples while recording, on the audio thread; must not block.
pub type ChunkSink = Arc<dyn Fn(Vec<f32>) + Send + Sync>;
/// Why a recording ended on its own: it is full, its time is up with no audio, its stream failed or its tick panicked.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum End {
    Cap,
    Deadline,
    Broken,
    TickFailed,
}

/// What a recording lost: the stream failed, the ring overflowed or a tick panicked (the rest went unheard). Words from a lossy recording are never inserted.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Loss {
    Broken,
    Overflow,
    Failed,
}

/// What the audio thread hands over when a recording ends: the last samples, the total recorded and what was lost.
pub struct Heard {
    pub tail: Vec<f32>,
    pub total: usize,
    pub loss: Option<Loss>,
}

impl Heard {
    /// A recording that delivered nothing.
    fn nothing() -> Heard {
        Heard { tail: Vec::new(), total: 0, loss: None }
    }
}

/// Runs on the audio thread when recording ends, with what was heard.
type Done = Box<dyn FnOnce(Heard) + Send>;

/// Options for one recording, read from the user's settings when it starts.
pub struct Config {
    pub mic: Option<String>,
    pub max_secs: u64,
    pub linger: Duration,
    pub sink: ChunkSink,
    /// Runs once on the audio thread when the recording ends by itself (see `End`), so the caller can end it instead of losing what comes next.
    pub on_end: Arc<dyn Fn(End) + Send + Sync>,
    /// Runs once on the audio thread when the microphone could not be opened (already reported), so the caller can end the recording it started.
    pub on_fail: Arc<dyn Fn() + Send + Sync>,
}

/// A job the audio thread runs once, at its deadline.
type Job = Box<dyn FnOnce() + Send>;

/// What other threads send the audio thread: start a recording, stop it, or run a job at a time.
enum Cmd {
    Start(Config),
    Stop(Done),
    After(Instant, Job),
}

/// Resamples and forwards audio while a recording runs.
struct Chunker {
    rec: Arc<Recording>,
    resampler: Resampler,
    sink: ChunkSink,
    on_end: Arc<dyn Fn(End) + Send + Sync>,
    on_error: ErrorSink,
    shared: Arc<Shared>,
    /// When the recording must end even if no audio arrives: a dead stream delivers no samples, so the sample limit alone would never be reached.
    deadline: Instant,
    ended: bool,
    loss: Option<Loss>,
    overflow_told: bool,
    broken_told: bool,
    total: usize,
    next: Instant,
}

impl Chunker {
    /// Sends the audio recorded since the last tick to the sink and arms the next tick; `total` counts a chunk only once the sink took it, so a sink that panics leaves it uncounted; calls `on_end` once when the recording must end and tells
    /// the user once if the ring ever overflowed (audio was lost, never silently).
    fn tick(&mut self) {
        if self.failed() {
            return;
        }
        let drained = self.rec.drain();
        self.note_overflow(drained.overflowed);
        self.note_broken();
        let out = self.resampler.push(drained.samples);
        let sent = out.len();
        if sent > 0 {
            (self.sink)(out);
        }
        self.total += sent;
        self.next = Instant::now() + CHUNK_EVERY;
        if let Some(end) = self.ending().filter(|_| !std::mem::replace(&mut self.ended, true)) {
            (self.on_end)(end);
        }
    }

    /// Whether a tick panicked (`tick_or_fail`): the ring is stopped and the resampler is not trusted again, so the recording only waits for its Stop.
    fn failed(&self) -> bool {
        self.loss == Some(Loss::Failed)
    }

    /// Why the recording has to end now, if it does: its stream failed (device unplugged), it is full, or its time is up although no audio arrived.
    fn ending(&self) -> Option<End> {
        if self.shared.broken.load(Ordering::Relaxed) {
            Some(End::Broken)
        } else if self.rec.full() {
            Some(End::Cap)
        } else {
            (Instant::now() >= self.deadline).then_some(End::Deadline)
        }
    }

    /// Tells the user once that the stream failed (the cpal error callback only sets the flag; formatting and reporting happen here, off the realtime thread).
    fn note_broken(&mut self) {
        if !self.shared.broken.load(Ordering::Relaxed) {
            return;
        }
        self.loss = Some(Loss::Broken);
        if !std::mem::replace(&mut self.broken_told, true) {
            (self.on_error)("The microphone stopped working. Check that it is still connected.".into());
        }
    }

    /// Tells the user once that the ring overflowed and audio was lost. Input: whether the latest drain reported an overflow.
    fn note_overflow(&mut self, overflowed: bool) {
        if overflowed {
            self.loss.get_or_insert(Loss::Overflow);
        }
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

/// Ends the recording; `done(heard)` runs on the audio thread. If that thread is not there (no recording ever started, or it retired or died) it runs here with nothing, so whoever waits
/// for it (the overlay, the speech session) is never left hanging.
pub fn stop(done: impl FnOnce(Heard) + Send + 'static) {
    if let Err(Cmd::Stop(done)) = TX.try_send(Cmd::Stop(Box::new(done))) {
        done(Heard::nothing());
    }
}

/// Runs `job` on the audio thread once `delay` has passed, and the thread does not retire before. If that thread is not there it runs here at once, like `stop`, so the caller is never left waiting. Inputs: the delay and the job.
pub fn after(delay: Duration, job: impl FnOnce() + Send + 'static) {
    if let Err(Cmd::After(_, job)) = TX.try_send(Cmd::After(Instant::now() + delay, Box::new(job))) {
        job();
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
        match open_stream(shared, &config.mic) {
            Ok(o) => *open = Some(o),
            Err(e) => on_error(e.to_string()),
        }
    }
    let o = open.as_ref()?;
    let resampler = Resampler::new(o.rate).map_err(|e| on_error(e)).ok()?;
    o.rec.start(o.rate as usize * config.max_secs as usize);
    Some(Chunker {
        rec: o.rec.clone(),
        resampler,
        sink: config.sink.clone(),
        on_end: config.on_end.clone(),
        on_error: on_error.clone(),
        shared: shared.clone(),
        deadline: Instant::now() + Duration::from_secs(config.max_secs) + DEADLINE_SLACK,
        ended: false,
        loss: None,
        overflow_told: false,
        broken_told: false,
        total: 0,
        next: Instant::now() + CHUNK_EVERY,
    })
}

/// Ends a recording: sends the last samples to `done`. A panic on the way (a problem sink, the resampler) still completes `done`, with nothing,
/// so whoever waits for the text is never left hanging. Input: the chunker if one was running, and the callback.
fn finish(chunker: Option<Chunker>, done: Done) {
    let pending = Cell::new(Some(done));
    if crate::guard::contain(|| drain_into(chunker, &pending)).is_none() {
        if let Some(done) = pending.take() {
            done(Heard::nothing());
        }
    }
}

/// The body of `finish`: the last samples go to the callback while it is still in `pending`. Inputs: the chunker and the callback's cell.
fn drain_into(chunker: Option<Chunker>, pending: &Cell<Option<Done>>) {
    let Some(mut c) = chunker else { return pending.take().map_or((), |done| done(Heard::nothing())) };
    if c.failed() {
        return pending.take().map_or((), |done| done(Heard { tail: Vec::new(), total: c.total, loss: c.loss }));
    }
    let drained = c.rec.stop();
    c.note_overflow(drained.overflowed);
    c.note_broken(); // a stream that failed after the last tick must not vanish with the chunker
    let mut tail = c.resampler.push(drained.samples);
    tail.extend(c.resampler.finish());
    let total = c.total + tail.len();
    if let Some(done) = pending.take() {
        done(Heard { tail, total, loss: c.loss });
    }
}

/// Ticks the running recording. A panic on the way (the sink, the resampler) ends it: the chunker is marked `Loss::Failed` and its ring stops filling, the problem is reported and the owner is told `End::TickFailed`, so it ends the recording like any other end; an owner already told of an end (the panic was in that `on_end`) is not told twice. The later Stop completes with the samples the engine already got, so the words recognised so far are rescued. Inputs: the chunker slot and the problem sink.
fn tick_or_fail(chunker: &mut Option<Chunker>, on_error: &ErrorSink) {
    let Some(c) = chunker.as_mut() else { return };
    if crate::guard::contain(|| c.tick()).is_some() {
        return;
    }
    c.loss = Some(Loss::Failed);
    let (on_end, told) = (c.on_end.clone(), c.ended);
    c.rec.stop();
    let _ = crate::guard::contain(|| on_error("The microphone recording failed.".into()));
    if !told {
        let _ = crate::guard::contain(|| on_end(End::TickFailed));
    }
}

/// Handles `Cmd::Stop`: completes the recording (a failed one with what it sent before the failure) and schedules the device to close after `linger`. Inputs: the chunker slot, the callback, the linger and the close deadline to set.
fn end_recording(chunker: &mut Option<Chunker>, done: Done, linger: Duration, close_at: &mut Option<Instant>) {
    finish(chunker.take(), done);
    *close_at = Some(Instant::now() + linger);
}

/// Runs, each under `guard::contain`, the jobs whose deadline has passed, and keeps the rest. Input: the pending jobs.
fn run_due(timers: &mut Vec<(Instant, Job)>) {
    let now = Instant::now();
    let (due, later): (Vec<_>, Vec<_>) = std::mem::take(timers).into_iter().partition(|(at, _)| *at <= now);
    *timers = later;
    for (_, job) in due {
        let _ = crate::guard::contain(job);
    }
}

/// The audio thread: blocks until a command, the next chunk time (recording only), a pending job or the linger deadline, so nothing polls while idle. When the linger ends it drops the stream, which releases the device; it retires only with no job pending. Input: the command channel, the shared flags and the problem sink.
fn run(rx: mpsc::Receiver<Cmd>, shared: Arc<Shared>, on_error: ErrorSink) {
    let mut open: Option<Open> = None;
    let (mut close_at, mut linger) = (None::<Instant>, Duration::ZERO);
    let mut chunker: Option<Chunker> = None;
    let mut timers: Vec<(Instant, Job)> = Vec::new();
    loop {
        let wake = [close_at, chunker.as_ref().filter(|c| !c.failed()).map(|c| c.next)]
            .into_iter()
            .chain(timers.iter().map(|t| Some(t.0)))
            .flatten()
            .min();
        let cmd = match wake {
            Some(t) => rx.recv_timeout(t.saturating_duration_since(Instant::now())),
            None => rx.recv().map_err(|_| RecvTimeoutError::Disconnected),
        };
        run_due(&mut timers);
        let cmd = match cmd {
            Err(RecvTimeoutError::Timeout) if chunker.is_none() && close_at.is_none_or(|t| t <= Instant::now()) => {
                (open, close_at) = (None, None);
                shared.forget();
                crate::mem::trim_soon();
                if !timers.is_empty() {
                    continue;
                }
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
            Ok(Cmd::Stop(done)) => end_recording(&mut chunker, done, linger, &mut close_at),
            Ok(Cmd::After(at, job)) => timers.push((at, job)),
            Err(RecvTimeoutError::Timeout) => {
                tick_or_fail(&mut chunker, &on_error);
            }
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}
