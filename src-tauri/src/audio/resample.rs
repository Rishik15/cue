//! Purpose: Convert captured audio to the 16 kHz mono f32 that speech engines expect.
//! Contents: to_16k — rubato FFT resampler in fixed chunks with a zero-padded tail.

use rubato::{FftFixedIn, Resampler};

use super::RATE;

const CHUNK: usize = 1024;

/// Resample mono audio to 16 kHz with rubato's FFT resampler (fixed-size chunks, zero-padded tail).
pub fn to_16k(input: &[f32], rate: u32) -> Vec<f32> {
    if rate == RATE || input.is_empty() {
        return input.to_vec();
    }
    let Ok(mut r) = FftFixedIn::<f32>::new(rate as usize, RATE as usize, CHUNK, 2, 1) else { return Vec::new() };
    let mut out = Vec::with_capacity(input.len() * RATE as usize / rate as usize + CHUNK);
    let mut pos = 0;
    while input.len() - pos >= r.input_frames_next() {
        let n = r.input_frames_next();
        if let Ok(o) = r.process(&[&input[pos..pos + n]], None) {
            out.extend_from_slice(&o[0]);
        }
        pos += n;
    }
    if pos < input.len() {
        if let Ok(o) = r.process_partial(Some(&[&input[pos..]]), None) {
            out.extend_from_slice(&o[0]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resamples_48k_to_16k() {
        let sine: Vec<f32> = (0..48_000).map(|i| (i as f32 * 440.0 * std::f32::consts::TAU / 48_000.0).sin()).collect();
        let out = to_16k(&sine, 48_000);
        assert!((out.len() as i64 - 16_000).abs() < 1_200, "got {}", out.len());
        assert!(out.iter().any(|s| s.abs() > 0.5));
        assert_eq!(to_16k(&sine, RATE).len(), sine.len());
    }
}
