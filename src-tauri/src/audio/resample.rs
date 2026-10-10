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

    /// Resamples every complete chunk; the remainder waits for the next call.
    pub fn push(&mut self, input: &[f32]) -> Vec<f32> {
        let Some(r) = self.inner.as_mut() else { return input.to_vec() };
        self.pending.extend_from_slice(input);
        let mut out = Vec::with_capacity(self.pending.len() * RATE as usize / 40_000 + CHUNK);
        while self.pending.len() >= r.input_frames_next() {
            let n = r.input_frames_next();
            if let Ok(o) = r.process(&[&self.pending[..n]], None) {
                out.extend_from_slice(&o[0]);
            }
            self.pending.drain(..n);
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(n: usize) -> Vec<f32> {
        (0..n).map(|i| (i as f32 * 440.0 * std::f32::consts::TAU / 48_000.0).sin()).collect()
    }

    #[test]
    fn resamples_48k_to_16k_in_pieces() {
        let input = sine(48_000);
        let mut r = Resampler::new(48_000).unwrap();
        let mut out: Vec<f32> = input.chunks(9_600).flat_map(|c| r.push(c)).collect(); // 200 ms pieces, as the recorder delivers them
        out.extend(r.finish());
        assert!((out.len() as i64 - 16_000).abs() < 1_200, "got {}", out.len());
        assert!(out.iter().any(|s| s.abs() > 0.5));
    }

    #[test]
    fn sixteen_khz_passes_through() {
        let input = sine(1_000);
        let mut r = Resampler::new(RATE).unwrap();
        assert_eq!(r.push(&input), input);
        assert!(r.finish().is_empty());
    }
}
