//! Purpose: Open the chosen (or default) microphone and feed the shared recording buffer from cpal's realtime callback.
//! Contents: Shared — buffer, recording flag, cap and level shared with the callback; Open — a running stream;
//! open_stream — builds and starts the stream for the device's native format; input_device_names — for the settings
//! list; callback — downmix, append (capped), publish level and the live flag.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample, Stream};

use super::ErrorSink;

/// State shared with the realtime callback: kept to a short mutex hold and two atomics.
pub struct Shared {
    pub buf: Mutex<Vec<f32>>,
    pub recording: AtomicBool,
    /// True once the stream has delivered audio for the current recording; the overlay stays grey until then.
    pub live: AtomicBool,
    pub level: AtomicU32, // f32 bits, RMS of the last callback
    pub max_samples: AtomicUsize,
    /// Samples captured in this recording (the buffer is emptied every few hundred ms, so its length is not the total).
    pub recorded: AtomicUsize,
    /// The stream reported an error (device unplugged, driver reset); it is reopened at the next recording.
    pub broken: AtomicBool,
}

/// A running input stream; dropping it releases the device.
pub struct Open {
    _stream: Stream,
    pub rate: u32,
    pub mic: Option<String>,
}

impl Shared {
    pub fn new() -> Arc<Self> {
        Arc::new(Shared {
            buf: Mutex::new(Vec::new()),
            recording: AtomicBool::new(false),
            live: AtomicBool::new(false),
            level: AtomicU32::new(0),
            max_samples: AtomicUsize::new(0),
            recorded: AtomicUsize::new(0),
            broken: AtomicBool::new(false),
        })
    }
}

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

pub fn open_stream(shared: &Arc<Shared>, on_error: &ErrorSink, mic: &Option<String>) -> Result<Open, String> {
    let device = pick_device(mic).ok_or("No microphone found.")?;
    let config = device.default_input_config().map_err(|e| format!("Microphone unavailable: {e}"))?;
    let (rate, channels) = (config.sample_rate().0, config.channels() as usize);
    let cfg = config.clone().into();
    let sink = on_error.clone();
    let flag = shared.clone();
    let err = move |e| {
        flag.broken.store(true, Ordering::Relaxed);
        sink(format!("Microphone error: {e}"));
    };
    let stream = match config.sample_format() {
        SampleFormat::F32 => device.build_input_stream(&cfg, callback::<f32>(shared.clone(), channels), err, None),
        SampleFormat::I16 => device.build_input_stream(&cfg, callback::<i16>(shared.clone(), channels), err, None),
        SampleFormat::U16 => device.build_input_stream(&cfg, callback::<u16>(shared.clone(), channels), err, None),
        f => return Err(format!("Unsupported microphone format {f:?}.")),
    }
    .map_err(|e| format!("Microphone busy or blocked: {e}"))?;
    stream.play().map_err(|e| format!("Microphone could not start: {e}"))?;
    Ok(Open { _stream: stream, rate, mic: mic.clone() })
}

/// Realtime callback: downmix to mono, append while recording (capped by the user's limit), publish the level.
fn callback<T: SizedSample>(shared: Arc<Shared>, channels: usize) -> impl FnMut(&[T], &cpal::InputCallbackInfo) + Send + 'static
where
    f32: FromSample<T>,
{
    move |data, _| {
        if !shared.recording.load(Ordering::Acquire) {
            return;
        }
        shared.live.store(true, Ordering::Relaxed);
        let cap = shared.max_samples.load(Ordering::Relaxed);
        let mut have = shared.recorded.load(Ordering::Relaxed);
        let mut buf = shared.buf.lock().unwrap();
        let (mut sum, mut n) = (0.0f32, 0usize);
        for frame in data.chunks_exact(channels) {
            let m = frame.iter().map(|s| f32::from_sample(*s)).sum::<f32>() / channels as f32;
            sum += m * m;
            n += 1;
            if have < cap {
                buf.push(m);
                have += 1;
            }
        }
        shared.recorded.store(have, Ordering::Relaxed);
        if n > 0 {
            shared.level.store((sum / n as f32).sqrt().to_bits(), Ordering::Relaxed);
        }
    }
}
