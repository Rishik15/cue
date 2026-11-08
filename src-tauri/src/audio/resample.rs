//! Purpose: Convert captured audio to the 16 kHz mono f32 that speech engines expect, a chunk at a time while recording.
//! Contents: Resampler — streaming rubato FFT resampler: push whole chunks as audio arrives, finish with a zero-padded tail.

use rubato::{FftFixedIn, Resampler as _};

use super::RATE;

const CHUNK: usize = 1024;

pub struct Resampler {
    /// None when the device already runs at 16 kHz.
    inner: Option<FftFixedIn<f32>>,
    pending: Vec<f32>,
}

impl Resampler {
    pub fn new(rate: u32) -> Result<Self, String> {
        let inner = match rate == RATE {
            true => None,
            false => Some(FftFixedIn::<f32>::new(rate as usize, RATE as usize, CHUNK, 2, 1).map_err(|e| format!("Resampler: {e}"))?),
        };
        Ok(Resampler { inner, pending: Vec::new() })
    }

    /// Resamples every complete chunk; the remainder waits for the next call. A 16 kHz input is returned as is, without a copy.
    pub fn push(&mut self, input: Vec<f32>) -> Vec<f32> {
        let Some(r) = self.inner.as_mut() else { return input };
        self.pending.extend_from_slice(&input);
        let mut out = Vec::with_capacity(self.pending.len() * RATE as usize / 40_000 + CHUNK);
        let mut at = 0;
        while self.pending.len() - at >= r.input_frames_next() {
            let n = r.input_frames_next();
            // A failed chunk stays in `pending` for the next call instead of being lost.
            let Ok(o) = r.process(&[&self.pending[at..at + n]], None) else { break };
            out.extend_from_slice(&o[0]);
            at += n;
        }
        self.pending.drain(..at);
        out
    }

    /// The last partial chunk, zero padded by rubato.
    pub fn finish(&mut self) -> Vec<f32> {
        let Some(r) = self.inner.as_mut() else { return Vec::new() };
        if self.pending.is_empty() {
            return Vec::new();
        }
        let tail = std::mem::take(&mut self.pending);
        r.process_partial(Some(&[&tail]), None).map(|o| o[0].clone()).unwrap_or_default()
    }
}
