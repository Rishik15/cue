//! File: audio/resample.rs
//! Purpose: Convert captured audio to the 16 kHz mono f32 that speech engines expect, a chunk at a time while recording.
//! Contents: Resampler — streaming rubato FFT resampler: push whole chunks as audio arrives, finish by flushing the tail so the output has exactly
//! the length the input implies.

use rubato::{FftFixedIn, Resampler as _};

use super::RATE;

const CHUNK: usize = 1024;

pub struct Resampler {
    /// None when the device already runs at 16 kHz.
    inner: Option<FftFixedIn<f32>>,
    pending: Vec<f32>,
    rate: u32,
    taken: usize,
    given: usize,
}

impl Resampler {
    /// A resampler from the device rate to 16 kHz. Input: the device rate. Returns it, or why rubato refused the rate.
    pub fn new(rate: u32) -> Result<Self, String> {
        let inner = match rate == RATE {
            true => None,
            false => Some(FftFixedIn::<f32>::new(rate as usize, RATE as usize, CHUNK, 2, 1).map_err(|e| format!("Resampler: {e}"))?),
        };
        Ok(Resampler { inner, pending: Vec::new(), rate, taken: 0, given: 0 })
    }

    /// Resamples every complete chunk; the remainder waits for the next call. A 16 kHz input is returned as is, without a copy.
    pub fn push(&mut self, input: Vec<f32>) -> Vec<f32> {
        self.taken += input.len();
        let Some(r) = self.inner.as_mut() else {
            self.given += input.len();
            return input;
        };
        self.pending.extend_from_slice(&input);
        let mut out = Vec::with_capacity(self.pending.len() * RATE as usize / 40_000 + CHUNK);
        let mut at = 0;
        while self.pending.len() - at >= r.input_frames_next() {
            let n = r.input_frames_next();
            let Ok(o) = r.process(&[&self.pending[at..at + n]], None) else { break };
            out.extend_from_slice(&o[0]);
            at += n;
        }
        self.pending.drain(..at);
        self.given += out.len();
        out
    }

    /// The rest of the audio: the last partial chunk padded with silence, which also pushes out what the filter still holds, cut to the length
    /// the whole input implies so nothing is lost and no padding is added.
    pub fn finish(&mut self) -> Vec<f32> {
        let Some(r) = self.inner.as_mut() else { return Vec::new() };
        let wanted = (self.taken as u64 * RATE as u64 / self.rate as u64) as usize;
        let tail = std::mem::take(&mut self.pending);
        let mut out = Vec::new();
        let mut flushed = if tail.is_empty() { r.process_partial::<&[f32]>(None, None) } else { r.process_partial(Some(&[&tail]), None) };
        for _ in 0..4 {
            let Ok(o) = flushed else { break };
            out.extend_from_slice(&o[0]);
            if self.given + out.len() >= wanted {
                break;
            }
            flushed = r.process_partial::<&[f32]>(None, None);
        }
        out.truncate(wanted.saturating_sub(self.given));
        self.given += out.len();
        out
    }
}
