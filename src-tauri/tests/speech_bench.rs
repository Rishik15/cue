//! File: tests/speech_bench.rs
//! Purpose: Time a dictation the way a user waits for it: key-down to first caption, Stop to final text, decode speed as a multiple of real time, cold and warm worker load, the worker's peak memory and CPU, and
//! word loss, over fixed 3, 10 and 30 s clips, at each requested thread count. Every later speech change (thread count, streaming profile, pause length) is judged by these numbers.
//! Contents: Config — what the environment asks for; Clip / build_clips — the fixed clips; Run / run_clips — the two passes over them; report — the p50 / p95 table.
//! Run with `scripts/cue.ps1 lab speech`. Skipped unless CUE_BENCH_MODELS ("folder@Family;...", Family is Transducer, Canary, Whisper or Streaming) and CUE_BENCH_SET (folder with refs.tsv, one
//! `id<TAB>reference text[<TAB>protected words separated by |]` per line, and `id.wav` 16 kHz mono 16-bit) are set. Optional: CUE_BENCH_THREADS ("1,2,4", default 2), CUE_BENCH_CLIPS (clips per length,
//! default 30), CUE_BENCH_RAW (use each utterance as one clip instead of joining them to 3, 10 and 30 s: for the word-loss corpus), CUE_BENCH_LABEL (tag printed on every line).
//! The audio is sent paced (a 200 ms piece every 200 ms, as the recorder does) for first caption and Stop to text, then again as fast as the pipe takes it for the decode speed.
//! Transcripts are never printed. Only the worker process is measured here; the app's own memory comes from `scripts/cue.ps1 bench` and the two add up.

mod common;

use std::time::Instant;

use common::stats::{errors, lost_words, percentile, plan_clips, words, Errors};
use common::{wav_samples, Worker};

/// The clip lengths the bench makes, in seconds.
const TARGET_SECS: [f32; 3] = [3.0, 10.0, 30.0];
const RATE: f32 = 16_000.0;

/// What the environment asks for.
struct Config {
    models: Vec<(String, String)>,
    set: String,
    threads: Vec<i32>,
    clips: usize,
    raw: bool,
    label: String,
}

/// One fixed clip: its audio, what it should say and the words that must survive.
struct Clip {
    bucket: String,
    audio: Vec<f32>,
    reference: Vec<String>,
    protected: Vec<String>,
}

/// What one clip measured.
struct Sample {
    bucket: String,
    first_caption_ms: Option<f64>,
    stop_to_text_ms: f64,
    real_time_factor: f64,
    errors: Errors,
    lost: usize,
    reference_words: usize,
}

/// Reads the configuration from the environment. Returns None (the bench is skipped) when the models or the set are not named.
fn config() -> Option<Config> {
    let (models, set) = (std::env::var("CUE_BENCH_MODELS").ok()?, std::env::var("CUE_BENCH_SET").ok()?);
    let list = |name: &str, default: &str| std::env::var(name).unwrap_or_else(|_| default.to_string());
    let models =
        models.split(';').map(|m| m.split_once('@').expect("folder@Family")).map(|(d, f)| (d.to_string(), f.to_string())).collect();
    Some(Config {
        models,
        set,
        threads: list("CUE_BENCH_THREADS", "2").split(',').map(|t| t.trim().parse().expect("a thread count")).collect(),
        clips: list("CUE_BENCH_CLIPS", "30").parse().expect("a clip count"),
        raw: std::env::var("CUE_BENCH_RAW").is_ok(),
        label: list("CUE_BENCH_LABEL", ""),
    })
}

/// Reads the set: each utterance's audio, reference words and protected words. Input: the set folder.
fn read_set(dir: &str) -> Vec<(Vec<f32>, Vec<String>, Vec<String>)> {
    let refs = std::fs::read_to_string(format!("{dir}/refs.tsv")).expect("refs.tsv");
    refs.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|line| {
            let mut cols = line.split('\t');
            let (id, text) = (cols.next().unwrap(), cols.next().expect("a reference text"));
            let protected = cols.next().map(|p| p.split('|').flat_map(words).collect()).unwrap_or_default();
            (wav_samples(&std::fs::read(format!("{dir}/{id}.wav")).expect("wav")), words(text), protected)
        })
        .collect()
}

/// The fixed clips of the set: either every utterance as it is, or utterances joined to each target length (the same ones every run). Inputs: the set and the config.
fn build_clips(config: &Config) -> Vec<Clip> {
    let set = read_set(&config.set);
    if config.raw {
        return set.into_iter().map(|(audio, reference, protected)| Clip { bucket: "raw".into(), audio, reference, protected }).collect();
    }
    let lengths: Vec<f32> = set.iter().map(|u| u.0.len() as f32 / RATE).collect();
    let set = &set;
    TARGET_SECS
        .iter()
        .flat_map(|target| {
            plan_clips(&lengths, *target, config.clips).into_iter().map(move |picks| Clip {
                bucket: format!("{target:.0}s"),
                audio: picks.iter().flat_map(|i| set[*i].0.iter().copied()).collect(),
                reference: picks.iter().flat_map(|i| set[*i].1.iter().cloned()).collect(),
                protected: picks.iter().flat_map(|i| set[*i].2.iter().cloned()).collect(),
            })
        })
        .collect()
}

/// Both passes over the clips on one loaded worker. Inputs: the worker and the clips. Returns one sample per clip.
fn run_clips(worker: &mut Worker, clips: &[Clip]) -> Vec<Sample> {
    clips
        .iter()
        .enumerate()
        .map(|(n, clip)| {
            let id = n as u64 * 2 + 1;
            let paced = worker.dictate(id, &clip.audio, true);
            let fast = worker.dictate(id + 1, &clip.audio, false);
            let heard = words(&paced.text);
            Sample {
                bucket: clip.bucket.clone(),
                first_caption_ms: paced.partials.first().map(|p| p.0.as_secs_f64() * 1e3),
                stop_to_text_ms: paced.stop_to_text.as_secs_f64() * 1e3,
                real_time_factor: f64::from(clip.audio.len() as f32 / RATE) / fast.total.as_secs_f64(),
                errors: errors(&clip.reference, &heard),
                lost: lost_words(&clip.protected, &heard),
                reference_words: clip.reference.len(),
            }
        })
        .collect()
}

/// Prints one line of the table for a bucket's samples. Inputs: the line's tag, the samples, and the run's label.
fn report(tag: &str, samples: &[&Sample], label: &str) {
    let col = |f: &dyn Fn(&Sample) -> Option<f64>| samples.iter().filter_map(|s| f(s)).collect::<Vec<_>>();
    let (first, stop, rtf) = (col(&|s| s.first_caption_ms), col(&|s| Some(s.stop_to_text_ms)), col(&|s| Some(s.real_time_factor)));
    let total: Errors = samples.iter().fold(Errors::default(), |a, s| Errors {
        substitutions: a.substitutions + s.errors.substitutions,
        insertions: a.insertions + s.errors.insertions,
        deletions: a.deletions + s.errors.deletions,
    });
    let words: usize = samples.iter().map(|s| s.reference_words).sum();
    let lost: usize = samples.iter().map(|s| s.lost).sum();
    eprintln!(
        "RESULT {label} {tag} clips={} first_caption_ms p50={:.0} p95={:.0} (none: {}) stop_to_text_ms p50={:.0} p95={:.0} rtf p50={:.1} p5={:.1} WER={:.1}% deleted={} of {words} protected_lost={lost}",
        samples.len(),
        percentile(&first, 50.0),
        percentile(&first, 95.0),
        samples.len() - first.len(),
        percentile(&stop, 50.0),
        percentile(&stop, 95.0),
        percentile(&rtf, 50.0),
        percentile(&rtf, 5.0),
        100.0 * total.total() as f64 / words.max(1) as f64,
        total.deletions,
    );
}

/// Measures one model at one thread count and prints its table. Inputs: the model folder, family, thread count and config.
fn measure(dir: &str, family: &str, threads: i32, config: &Config, clips: &[Clip]) {
    let cold = Worker::start(dir, family, threads);
    let (cold_load, cold_load_inside) = (cold.load, cold.reported_load_ms);
    let first = cold_first_dictation(cold, clips.first());
    let mut worker = Worker::start(dir, family, threads);
    let (warm_load, idle_mb) = (worker.load, worker.private_mb());
    let cpu_before = worker.cpu_secs();
    let samples = run_clips(&mut worker, clips);
    let audio_secs: f32 = clips.iter().map(|c| c.audio.len() as f32 / RATE).sum();
    let tag = format!("{dir} threads={threads}");
    eprintln!(
        "RESULT {} {tag} load cold={} ms (worker says {cold_load_inside}) warm={} ms; first dictation on a cold worker: stop_to_text={first:.0} ms; private MB loaded={idle_mb:.0} after={:.0}; cpu {:.2} s per audio s (both passes)",
        config.label,
        cold_load.as_millis(),
        warm_load.as_millis(),
        worker.private_mb(),
        (worker.cpu_secs() - cpu_before) / f64::from(audio_secs.max(1.0)),
    );
    let mut buckets: Vec<&str> = samples.iter().map(|s| s.bucket.as_str()).collect();
    buckets.dedup();
    for bucket in buckets {
        let of: Vec<&Sample> = samples.iter().filter(|s| s.bucket == bucket).collect();
        report(&format!("{tag} {bucket}"), &of, &config.label);
    }
    worker.finish();
}

/// The first dictation on a freshly loaded worker, which pays for lazily built state: its Stop-to-text in ms. Inputs: the worker (closed after) and the first clip.
fn cold_first_dictation(mut worker: Worker, clip: Option<&Clip>) -> f64 {
    let ms = clip.map_or(0.0, |c| worker.dictate(1, &c.audio, true).stop_to_text.as_secs_f64() * 1e3);
    worker.finish();
    ms
}

#[test]
#[ignore = "measures real models; run with scripts/cue.ps1 lab speech"]
fn measure_dictation_timing() {
    let Some(config) = config() else {
        eprintln!("skipped: CUE_BENCH_MODELS / CUE_BENCH_SET not set");
        return;
    };
    let clips = build_clips(&config);
    assert!(!clips.is_empty(), "the set made no clips");
    let started = Instant::now();
    for (dir, family) in &config.models {
        for threads in &config.threads {
            measure(dir, family, *threads, &config, &clips);
        }
    }
    eprintln!("RESULT {} done in {:.0} s", config.label, started.elapsed().as_secs_f32());
}
