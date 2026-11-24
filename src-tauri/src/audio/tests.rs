//! File: audio/tests.rs
//! Purpose: Protect the capture path without a microphone: the resampler's length and chunk-partition independence at the common device rates,
//! a stream that failed or went silent ending its recording, the startup delay removed so a burst at the end keeps its place, the downmix and level for 1, 2 and 6 channels, the tail kept on stop, a callback that was running across a stop or restart being rejected, and
//! the ring's overflow and recording limit, the reason a recording ends and what it lost.
//! Contents: tests of `resample` and `recording`.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use proptest::prelude::*;

use super::capture::{OpenError, Shared};
use super::recording::Recording;
use super::resample::Resampler;
use super::RATE;
use super::{end_recording, finish, tick_or_fail, Chunker, End, ErrorSink, Heard, Loss};

/// A half-scale 440 Hz sine. Inputs: the sample rate and the number of samples.
fn tone(rate: u32, samples: usize) -> Vec<f32> {
    (0..samples).map(|n| (n as f32 * 440.0 * std::f32::consts::TAU / rate as f32).sin() * 0.5).collect()
}

/// Resamples `input` in callbacks of `piece` samples and finishes. Inputs: the device rate, the audio and the piece size. Returns all output.
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

/// A short burst at the very end of a recording keeps its place and its energy: the resampler's startup delay is removed instead of cutting the
/// same amount of real audio off the tail, at the common device rates and however the input was cut into callbacks.
#[test]
fn a_burst_at_the_end_keeps_its_position_and_energy() {
    for rate in [44_100u32, 48_000] {
        let burst = rate as usize / 50;
        let mut input = vec![0.0; rate as usize];
        input.extend(vec![0.5; burst]);
        for piece in [rate as usize / 100, input.len()] {
            let out = resample_in(rate, &input, piece);
            let (start, end) = (RATE as usize + 8, out.len() - 8);
            let mean = out[start..end].iter().sum::<f32>() / (end - start) as f32;
            assert!((mean - 0.5).abs() < 0.05, "{rate} Hz in pieces of {piece}: burst mean {mean}");
            let before = out[..RATE as usize - 8].iter().map(|s| s.abs()).fold(0.0, f32::max);
            assert!(before < 0.05, "{rate} Hz in pieces of {piece}: audio before the burst {before}");
        }
    }
}

/// A chunker over `rec` with the given end callback, problem sink, sink of audio and deadline.
fn chunker(
    rec: Arc<Recording>,
    shared: Arc<Shared>,
    on_end: impl Fn(End) + Send + Sync + 'static,
    on_error: ErrorSink,
    deadline: Instant,
) -> Chunker {
    Chunker {
        rec,
        resampler: Resampler::new(RATE).expect("resampler"),
        sink: Arc::new(|_| {}),
        on_end: Arc::new(on_end),
        on_error,
        shared,
        deadline,
        ended: false,
        loss: None,
        overflow_told: false,
        broken_told: false,
        total: 0,
        next: Instant::now(),
    }
}

/// A recording whose stream failed (microphone unplugged) ends through the same callback as a full one, once, instead of waiting for samples that never come, and the problem is reported once by the audio thread (the error callback only sets the flag).
#[test]
fn a_failed_stream_ends_the_recording_once() {
    let rec = Recording::new(64);
    rec.start(1_000_000);
    let shared = Shared::new();
    let ended = Arc::new(Mutex::new(Vec::new()));
    let ends = ended.clone();
    let told = Arc::new(Mutex::new(Vec::new()));
    let reports = told.clone();
    let mut chunker = chunker(
        rec,
        shared.clone(),
        move |e| ends.lock().unwrap().push(e),
        Arc::new(move |m| reports.lock().unwrap().push(m)),
        Instant::now() + Duration::from_secs(3600),
    );
    chunker.tick();
    assert!(ended.lock().unwrap().is_empty(), "a healthy recording keeps going");
    shared.broken.store(true, Ordering::Relaxed);
    chunker.tick();
    chunker.tick();
    assert_eq!(*ended.lock().unwrap(), [End::Broken], "once");
    assert_eq!(told.lock().unwrap().len(), 1, "the owner tells the user once, not the stream's error callback: {:?}", told.lock().unwrap());
}

/// A recording past its deadline ends even when the stream is silent and not marked failed.
#[test]
fn a_silent_stream_ends_at_its_deadline() {
    let rec = Recording::new(64);
    rec.start(1_000_000);
    let ended = Arc::new(Mutex::new(Vec::new()));
    let ends = ended.clone();
    let mut chunker = chunker(rec, Shared::new(), move |e| ends.lock().unwrap().push(e), Arc::new(|_| {}), Instant::now());
    chunker.tick();
    assert_eq!(*ended.lock().unwrap(), [End::Deadline]);
}

/// A full ring ends the recording with `Cap`.
#[test]
fn a_full_recording_ends_with_cap() {
    let rec = Recording::new(64);
    rec.start(8);
    rec.write(&[0.1; 20], 1, |s| *s);
    let ended = Arc::new(Mutex::new(Vec::new()));
    let ends = ended.clone();
    let mut chunker =
        chunker(rec, Shared::new(), move |e| ends.lock().unwrap().push(e), Arc::new(|_| {}), Instant::now() + Duration::from_secs(3600));
    chunker.tick();
    chunker.tick();
    assert_eq!(*ended.lock().unwrap(), [End::Cap]);
}

/// What a recording lost reaches the owner with the last samples: a broken stream is `Broken`, an overflow `Overflow`, a clean one nothing, and a broken stream stays `Broken` after an overflow.
#[test]
fn the_last_samples_carry_what_was_lost() {
    let heard = |overflow: bool, broken: bool| {
        let rec = Recording::new(10);
        rec.start(1000);
        if overflow {
            rec.write(&[0.1; 25], 1, |s| *s);
        }
        let shared = Shared::new();
        shared.broken.store(broken, Ordering::Relaxed);
        let c = chunker(rec, shared, |_| {}, Arc::new(|_| {}), Instant::now() + Duration::from_secs(3600));
        let out = Arc::new(Mutex::new(None));
        let seen = out.clone();
        finish(Some(c), Box::new(move |h: Heard| *seen.lock().unwrap() = Some(h.loss)));
        let loss = out.lock().unwrap().take().expect("done ran");
        loss
    };
    assert_eq!(heard(false, false), None);
    assert_eq!(heard(true, false), Some(Loss::Overflow));
    assert_eq!(heard(false, true), Some(Loss::Broken));
    assert_eq!(heard(true, true), Some(Loss::Broken));
}

/// A panic inside a tick (here the audio sink) ends the recording through the owner with `TickFailed` and a report, without a manual Stop; the Stop that follows still completes with nothing.
#[test]
fn a_panicking_tick_tells_the_owner() {
    let rec = Recording::new(64);
    rec.start(1_000_000);
    rec.write(&[0.1; 200], 1, |s| *s);
    let handle = rec.clone();
    let ended = Arc::new(Mutex::new(Vec::new()));
    let ends = ended.clone();
    let told = Arc::new(Mutex::new(Vec::new()));
    let reports = told.clone();
    let mut c = chunker(
        rec,
        Shared::new(),
        move |e| ends.lock().unwrap().push(e),
        Arc::new(move |m| reports.lock().unwrap().push(m)),
        Instant::now() + Duration::from_secs(3600),
    );
    c.sink = Arc::new(|_| panic!("sink bug"));
    let mut slot = Some(c);
    let problems = Arc::new(Mutex::new(Vec::new()));
    let seen = problems.clone();
    let sink: ErrorSink = Arc::new(move |m| seen.lock().unwrap().push(m));
    assert!(handle.live());
    tick_or_fail(&mut slot, &sink);
    assert!(slot.is_none());
    assert!(!handle.live(), "the failed tick stopped the recording");
    assert_eq!(*ended.lock().unwrap(), [End::TickFailed]);
    assert_eq!(problems.lock().unwrap().len(), 1, "the failure was reported once");
    let completed = Arc::new(AtomicUsize::new(0));
    let done = completed.clone();
    let mut close_at = None;
    let before = Instant::now();
    end_recording(
        &mut slot,
        Box::new(move |heard| {
            assert!(heard.tail.is_empty() && heard.total == 0 && heard.loss.is_none());
            done.fetch_add(1, Ordering::Relaxed);
        }),
        Duration::from_secs(5),
        &mut close_at,
    );
    assert_eq!(completed.load(Ordering::Relaxed), 1, "the owner's Stop completes once with nothing");
    assert!(close_at.is_some_and(|t| t >= before + Duration::from_secs(5)), "the Stop arms the device closure");
}

/// A panic while a recording ends (here a problem sink) still completes the Stop callback, once and with nothing, instead of dropping it.
#[test]
fn a_panic_while_stopping_still_completes_stop() {
    let rec = Recording::new(64);
    rec.start(1_000_000);
    rec.write(&[0.1; 200], 1, |s| *s);
    let chunker = chunker(rec, Shared::new(), |_| {}, Arc::new(|_| panic!("sink bug")), Instant::now() + Duration::from_secs(3600));
    let completed = Arc::new(AtomicUsize::new(0));
    let seen = completed.clone();
    finish(
        Some(chunker),
        Box::new(move |heard| {
            assert!(heard.tail.is_empty() && heard.total == 0);
            seen.fetch_add(1, Ordering::Relaxed);
        }),
    );
    assert_eq!(completed.load(Ordering::Relaxed), 1);
}

/// Downmixes one frame the way the callback does. Inputs: the channel count and the frame. Returns the mono sample and the level.
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

/// A stream that failed after the last tick but before the queued Stop is still reported when the recording is drained, once, not dropped with the chunker.
#[test]
fn a_failure_before_stop_is_reported_while_draining() {
    let rec = Recording::new(64);
    rec.start(1_000_000);
    let shared = Shared::new();
    let told = Arc::new(Mutex::new(Vec::new()));
    let reports = told.clone();
    let chunker = chunker(
        rec,
        shared.clone(),
        |_| {},
        Arc::new(move |m| reports.lock().unwrap().push(m)),
        Instant::now() + Duration::from_secs(3600),
    );
    shared.broken.store(true, Ordering::Relaxed);
    finish(Some(chunker), Box::new(|_| {}));
    let reported = told.lock().unwrap().clone();
    assert_eq!(reported.len(), 1, "{reported:?}");
}

/// Each way the microphone can fail to open maps to the one sentence the user sees; the wording of the old string errors is kept.
#[test]
fn every_open_failure_has_its_one_user_message() {
    let messages = [
        (OpenError::NoDevice, "No microphone found."),
        (OpenError::Unavailable("gone".into()), "Microphone unavailable: gone"),
        (OpenError::Format("I8".into()), "Unsupported microphone format I8."),
        (OpenError::Busy("in use".into()), "Microphone busy or blocked: in use"),
        (OpenError::Start("denied".into()), "Microphone could not start: denied"),
    ];
    for (error, message) in messages {
        assert_eq!(error.to_string(), message);
    }
}
