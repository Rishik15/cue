//! File: audio/capture.rs
//! Purpose: Open the chosen (or default) microphone and feed its recording ring from cpal's realtime callback.
//! Contents: Shared — the latest ring and the broken flag; OpenError — why the device did not open (its Display is the one user message); Open — a running stream and its ring; open_stream — builds and starts the stream for the
//! device's native format; input_device_names — for the settings list; callback — hands the samples to `Recording::write`.

use crate::lock::lock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample, Stream};

use super::recording::Recording;

/// Seconds of audio the ring holds. Kept at 5 s: 960 KB of f32 at 48 kHz (320 KB at 16 kHz), against a drain every 200 ms, so the audio thread may be
/// late by 25 ticks before anything is lost. The drain is a copy, a resample of a few milliseconds and a channel send that never blocks, so only a
/// stall of seconds (a suspended machine, a debugger) overflows it, and then the overflow flag makes the user see "part of the recording was lost"
/// instead of silence. A 10 s ring would double the recording's private memory for a case that already reports itself; shrink this if the
/// `recording_private_mb` bench shows the 5 s ring regressing.
const RING_SECS: usize = 5;

/// State that outlives a stream: the recording ring of the open stream (for the overlay's level and live flag) and the broken flag.
pub struct Shared {
    rec: Mutex<Option<Arc<Recording>>>,
    /// The stream reported an error (device unplugged, driver reset); the audio thread tells the user and the stream is reopened at the next recording.
    pub broken: AtomicBool,
}

/// Why the microphone did not open. Its `Display` is the one sentence shown to the user (`audio::begin` reports it).
#[derive(Debug, PartialEq)]
pub enum OpenError {
    /// No input device at all.
    NoDevice,
    /// The device has no usable default configuration.
    Unavailable(String),
    /// The device delivers a sample format the capture cannot read.
    Format(String),
    /// Another program holds the device or Windows blocked it.
    Busy(String),
    /// The stream was built but would not start.
    Start(String),
}

impl std::fmt::Display for OpenError {
    /// Writes the sentence to tell the user.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OpenError::NoDevice => write!(f, "No microphone found."),
            OpenError::Unavailable(e) => write!(f, "Microphone unavailable: {e}"),
            OpenError::Format(format) => write!(f, "Unsupported microphone format {format}."),
            OpenError::Busy(e) => write!(f, "Microphone busy or blocked: {e}"),
            OpenError::Start(e) => write!(f, "Microphone could not start: {e}"),
        }
    }
}

/// A running input stream; dropping it releases the device.
pub struct Open {
    _stream: Stream,
    pub rate: u32,
    pub mic: Option<String>,
    pub rec: Arc<Recording>,
}

impl Shared {
    /// Flags shared with the overlay and the stream's error handler. Returns them ready to share.
    pub fn new() -> Arc<Self> {
        Arc::new(Shared { rec: Mutex::new(None), broken: AtomicBool::new(false) })
    }

    /// Lets go of the ring when its stream closes, so an idle Cue holds no audio memory.
    pub fn forget(&self) {
        *lock(&self.rec) = None;
    }

    /// The recording of the latest stream, if one was ever opened.
    pub fn rec(&self) -> Option<Arc<Recording>> {
        lock(&self.rec).clone()
    }
}

/// The names of the input devices; empty when enumeration fails.
pub fn input_device_names() -> Vec<String> {
    let devices = cpal::default_host().input_devices().map(|d| d.collect::<Vec<_>>()).unwrap_or_default();
    devices.iter().filter_map(|d| d.name().ok()).collect()
}

/// The named device if it is still plugged in, otherwise the system default.
fn pick_device(mic: &Option<String>) -> Option<cpal::Device> {
    let host = cpal::default_host();
    let named = mic.as_deref().and_then(|name| host.input_devices().ok()?.find(|d| d.name().is_ok_and(|n| n == name)));
    named.or_else(|| host.default_input_device())
}

/// Opens and starts the stream for `mic` (or the default). Inputs: the shared flags and the device name. Returns the stream or why it did not open.
pub fn open_stream(shared: &Arc<Shared>, mic: &Option<String>) -> Result<Open, OpenError> {
    let device = pick_device(mic).ok_or(OpenError::NoDevice)?;
    let config = device.default_input_config().map_err(|e| OpenError::Unavailable(e.to_string()))?;
    let (rate, channels) = (config.sample_rate().0, (config.channels() as usize).max(1));
    let cfg = config.clone().into();
    let rec = Recording::new(rate as usize * RING_SECS);
    let flag = shared.clone();
    let err = move |_| flag.broken.store(true, Ordering::Relaxed); // the owner reports it (`Chunker::tick`), never this realtime thread
    let stream = match config.sample_format() {
        SampleFormat::F32 => device.build_input_stream(&cfg, callback::<f32>(rec.clone(), channels), err, None),
        SampleFormat::I16 => device.build_input_stream(&cfg, callback::<i16>(rec.clone(), channels), err, None),
        SampleFormat::I32 => device.build_input_stream(&cfg, callback::<i32>(rec.clone(), channels), err, None),
        SampleFormat::U16 => device.build_input_stream(&cfg, callback::<u16>(rec.clone(), channels), err, None),
        f => return Err(OpenError::Format(format!("{f:?}"))),
    }
    .map_err(|e| OpenError::Busy(e.to_string()))?;
    stream.play().map_err(|e| OpenError::Start(e.to_string()))?;
    *lock(&shared.rec) = Some(rec.clone());
    Ok(Open { _stream: stream, rate, mic: mic.clone(), rec })
}

/// Realtime callback: hands each buffer to the recording, which downmixes and stores it without locking or allocating. Inputs: the ring and the channel count.
fn callback<T: SizedSample>(rec: Arc<Recording>, channels: usize) -> impl FnMut(&[T], &cpal::InputCallbackInfo) + Send + 'static
where
    f32: FromSample<T>,
{
    move |data, _| rec.write(data, channels, |s| f32::from_sample(*s))
}
