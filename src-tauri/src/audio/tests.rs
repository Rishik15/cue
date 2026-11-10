//! File: audio/tests.rs
//! Purpose: Protect the capture path without a microphone: the resampler's length and chunk-partition independence at the common device rates,
//! the downmix and level for 1, 2 and 6 channels, the tail kept on stop, a callback that was running across a stop or restart being rejected, and
//! the ring's overflow and recording limit.
//! Contents: tests of `resample` and `recording`.

use proptest::prelude::*;

use super::recording::Recording;
use super::resample::Resampler;
use super::RATE;

fn tone(rate: u32, samples: usize) -> Vec<f32> {
    (0..samples).map(|n| (n as f32 * 440.0 * std::f32::consts::TAU / rate as f32).sin() * 0.5).collect()
}

fn resample_in(rate: u32, input: &[f32], piece: usize) -> Vec<f32> {
    let mut r = Resampler::new(rate).expect("resampler");
    let mut out: Vec<f32> = input.chunks(piece).flat_map(|c| r.push(c.to_vec())).collect();
    out.extend(r.finish());
    out
}

/// Whatever the device rate, the output has exactly the length the input implies and does not depend on how the input was cut into callbacks.
#[test]
fn resampling_keeps_length_and_ignores_chunking() {
    for rate in [16_000u32, 44_100, 48_000] {
        let input = tone(rate, rate as usize + 777);
        let whole = resample_in(rate, &input, input.len());
        assert_eq!(whole.len(), input.len() * RATE as usize / rate as usize, "{rate} Hz");
        for piece in [1usize, 333, 1024, 4800] {
            assert_eq!(resample_in(rate, &input, piece), whole, "{rate} Hz in pieces of {piece}");
        }
    }
}

/// The tail is flushed on finish: a constant signal still reads as constant at the very end, instead of the last chunk being missing or silent.
#[test]
fn the_resampler_flushes_its_tail() {
    let out = resample_in(48_000, &vec![0.5; 48_000 + 500], 960);
    assert_eq!(out.len(), (48_000 + 500) / 3);
    assert!((out[out.len() - 1] - 0.5).abs() < 0.05, "{}", out[out.len() - 1]);
    assert!((out[out.len() / 2] - 0.5).abs() < 0.05);
}

fn mix(channels: usize, frame: &[f32]) -> (Vec<f32>, f32) {
    let rec = Recording::new(64);
    rec.start(64);
    rec.write(frame, channels, |s| *s);
    let level = rec.level();
    (rec.stop().samples, level)
}

/// Channels are averaged into mono and the level is the RMS of that mono signal, for mono, stereo and six-channel devices.
#[test]
fn downmix_and_level_for_one_two_and_six_channels() {
    assert_eq!(mix(1, &[0.5, -0.5]), (vec![0.5, -0.5], 0.5));
    let (stereo, level) = mix(2, &[1.0, 0.0, 0.0, 1.0]);
    assert_eq!(stereo, [0.5, 0.5]);
    assert!((level - 0.5).abs() < 1e-6);
    let (six, level) = mix(6, &[6.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
    assert_eq!(six, [1.0, 0.0]);
    assert!((level - (0.5f32).sqrt()).abs() < 1e-6);
    let (partial, _) = mix(2, &[1.0, 1.0, 0.25]);
    assert_eq!(partial, [1.0], "a half frame at the end of a buffer is ignored");
}

/// Stopping returns everything written so far, even what no drain had taken, and the callback after the stop adds nothing and stays not live.
#[test]
fn stop_keeps_the_tail_and_a_late_callback_adds_nothing() {
    let rec = Recording::new(64);
    rec.start(64);
    rec.write(&[0.1, 0.2, 0.3], 1, |s| *s);
    assert!(rec.live());
    assert_eq!(rec.drain().samples, [0.1, 0.2, 0.3]);
    rec.write(&[0.4, 0.5], 1, |s| *s);
    assert_eq!(rec.stop().samples, [0.4, 0.5]);
    rec.write(&[0.9], 1, |s| *s);
    assert!(!rec.live());
    assert!(rec.drain().samples.is_empty());
}

/// A callback that read "recording" and was held up across a stop and a new recording is rejected: it neither lands in the new recording nor
/// makes it live.
#[test]
fn a_callback_running_across_a_restart_is_rejected() {
    let rec = Recording::new(64);
    rec.start(64);
    let claim = rec.claim().expect("recording");
    let fill = rec.fill(&claim, &[0.7, 0.7], 1, |s| *s);
    assert!(rec.stop().samples.is_empty(), "the in-flight samples were never published");
    rec.start(64);
    assert!(!rec.commit(&claim, &fill));
    assert!(!rec.live());
    assert!(rec.drain().samples.is_empty());
    rec.write(&[0.3], 1, |s| *s);
    assert_eq!(rec.drain().samples, [0.3]);
}

/// A drain that falls behind by more than the ring holds loses the excess with an explicit overflow flag, and the ring keeps working after.
#[test]
fn the_ring_reports_overflow_and_never_grows() {
    let rec = Recording::new(10);
    rec.start(1000);
    rec.write(&(0..25).map(|n| n as f32).collect::<Vec<_>>(), 1, |s| *s);
    let drained = rec.drain();
    assert!(drained.overflowed);
    assert_eq!(drained.samples, (0..10).map(|n| n as f32).collect::<Vec<_>>());
    rec.write(&[50.0, 51.0], 1, |s| *s);
    assert_eq!(rec.drain().samples, [50.0, 51.0]);
}

/// The recording limit stops storing without an overflow report, and the recording says it is full.
#[test]
fn the_recording_limit_is_not_an_overflow() {
    let rec = Recording::new(100);
    rec.start(8);
    rec.write(&[0.1; 20], 1, |s| *s);
    assert!(rec.full());
    let drained = rec.drain();
    assert_eq!((drained.samples.len(), drained.overflowed), (8, false));
}

proptest! {
    /// For any device rate, input length and way of cutting the input into callbacks, `finish` never panics and the whole output has exactly the
    /// length the input implies.
    #[test]
    fn finish_gives_the_right_length_for_any_rate_and_chunking(
        rate in prop::sample::select(vec![8_000u32, 11_025, 16_000, 22_050, 32_000, 44_100, 48_000, 88_200, 96_000]),
        len in 0usize..30_000,
        piece in 1usize..5_000,
    ) {
        let input = tone(rate, len);
        let out = resample_in(rate, &input, piece);
        prop_assert_eq!(out.len(), len * RATE as usize / rate as usize);
        prop_assert!(out.iter().all(|s| s.is_finite()));
    }
}
