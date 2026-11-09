//! File: audio/capture.rs
//! Purpose: Open the chosen (or default) microphone and feed its recording ring from cpal's realtime callback.
//! Contents: Shared — the latest ring and the broken flag; Open — a running stream and its ring; open_stream — builds and starts the stream for the
//! device's native format; input_device_names — for the settings list; callback — hands the samples to `Recording::write`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample, Stream};

use super::recording::Recording;
use super::ErrorSink;

/// Seconds of audio the ring holds: twenty times the 200 ms drain period, so only an audio thread stalled for seconds overflows it.
const RING_SECS: usize = 5;

/// State that outlives a stream: the recording ring of the open stream (for the overlay's level and live flag) and the broken flag.
pub struct Shared {
    rec: Mutex<Option<Arc<Recording>>>,
    /// The stream reported an error (device unplugged, driver reset); it is reopened at the next recording.
    pub broken: AtomicBool,
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
        *self.rec.lock().unwrap_or_else(PoisonError::into_inner) = None;
    }

    /// The recording of the latest stream, if one was ever opened.
    pub fn rec(&self) -> Option<Arc<Recording>> {
        self.rec.lock().unwrap_or_else(PoisonError::into_inner).clone()
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

/// Opens and starts the stream for `mic` (or the default). Inputs: the shared flags, the problem sink and the device name. Returns the stream or the sentence to tell the user.
pub fn open_stream(shared: &Arc<Shared>, on_error: &ErrorSink, mic: &Option<String>) -> Result<Open, String> {
    let device = pick_device(mic).ok_or("No microphone found.")?;
    let config = device.default_input_config().map_err(|e| format!("Microphone unavailable: {e}"))?;
    let (rate, channels) = (config.sample_rate().0, (config.channels() as usize).max(1));
    let cfg = config.clone().into();
    let rec = Recording::new(rate as usize * RING_SECS);
    let sink = on_error.clone();
    let flag = shared.clone();
    let err = move |e| {
        flag.broken.store(true, Ordering::Relaxed);
        sink(format!("Microphone error: {e}"));
    };
    let stream = match config.sample_format() {
        SampleFormat::F32 => device.build_input_stream(&cfg, callback::<f32>(rec.clone(), channels), err, None),
        SampleFormat::I16 => device.build_input_stream(&cfg, callback::<i16>(rec.clone(), channels), err, None),
        SampleFormat::I32 => device.build_input_stream(&cfg, callback::<i32>(rec.clone(), channels), err, None),
        SampleFormat::U16 => device.build_input_stream(&cfg, callback::<u16>(rec.clone(), channels), err, None),
        f => return Err(format!("Unsupported microphone format {f:?}.")),
    }
    .map_err(|e| format!("Microphone busy or blocked: {e}"))?;
    stream.play().map_err(|e| format!("Microphone could not start: {e}"))?;
    *shared.rec.lock().unwrap_or_else(PoisonError::into_inner) = Some(rec.clone());
    Ok(Open { _stream: stream, rate, mic: mic.clone(), rec })
}

/// Realtime callback: hands each buffer to the recording, which downmixes and stores it without locking or allocating. Inputs: the ring and the channel count.
fn callback<T: SizedSample>(rec: Arc<Recording>, channels: usize) -> impl FnMut(&[T], &cpal::InputCallbackInfo) + Send + 'static
where
    f32: FromSample<T>,
{
    move |data, _| rec.write(data, channels, |s| f32::from_sample(*s))
}
