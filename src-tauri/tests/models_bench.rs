//! Purpose: Measure every speech model on the same real read speech (73 LibriSpeech utterances with reference text): word error rate,
//! decoding speed as a multiple of real time, load time and memory, so the ratings on the Models page come from numbers.
//! Run with `scripts/winsync models`; skipped unless CUE_BENCH_MODELS ("folder@Family;...") and CUE_BENCH_SET (folder with refs.tsv and WAVs) are set.

mod common;

use std::time::Instant;

use common::{wav_samples, Worker};

/// Lower case, words only (letters, digits, apostrophes): the usual minimal normalisation before comparing.
fn words(text: &str) -> Vec<String> {
    let spaced: String = text.to_lowercase().chars().map(|c| if c.is_alphanumeric() || c == '\'' { c } else { ' ' }).collect();
    spaced.split_whitespace().map(String::from).collect()
}

/// Word-level edit distance (substitutions, insertions, deletions).
fn edits(reference: &[String], heard: &[String]) -> usize {
    let mut row: Vec<usize> = (0..=heard.len()).collect();
    for (i, r) in reference.iter().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, h) in heard.iter().enumerate() {
            let above = row[j + 1];
            row[j + 1] = (diagonal + usize::from(r != h)).min(row[j] + 1).min(above + 1);
            diagonal = above;
        }
    }
    row[heard.len()]
}

#[test]
#[ignore = "measures real models; run with scripts/winsync models"]
fn measure_models() {
    let (Ok(models), Ok(set)) = (std::env::var("CUE_BENCH_MODELS"), std::env::var("CUE_BENCH_SET")) else {
        eprintln!("skipped: CUE_BENCH_MODELS / CUE_BENCH_SET not set");
        return;
    };
    let refs = std::fs::read_to_string(format!("{set}/refs.tsv")).expect("refs.tsv");
    let clips: Vec<(String, Vec<f32>, Vec<String>)> = refs
        .lines()
        .filter_map(|l| l.split_once('\t'))
        .map(|(id, text)| (id.to_string(), wav_samples(&std::fs::read(format!("{set}/{id}.wav")).expect("wav")), words(text)))
        .collect();
    let audio_secs: f32 = clips.iter().map(|c| c.1.len() as f32 / 16_000.0).sum();
    eprintln!("RESULT clips={} audio={audio_secs:.0}s", clips.len());
    for spec in models.split(';') {
        let (dir, family) = spec.split_once('@').expect("folder@Family");
        let loading = Instant::now();
        let mut worker = Worker::start(dir, family);
        let load_ms = loading.elapsed().as_millis();
        let (mut wrong, mut total, mut digits) = (0usize, 0usize, 0usize);
        let started = Instant::now();
        for (i, (_, audio, reference)) in clips.iter().enumerate() {
            let heard = worker.utterance(i as u64 + 1, audio);
            digits += usize::from(heard.chars().any(|c| c.is_ascii_digit()));
            let wrong_here = edits(reference, &words(&heard));
            if wrong_here > 0 && std::env::var("CUE_BENCH_SHOW").is_ok() {
                eprintln!("RESULT   diff {wrong_here}: REF {} | HEARD {heard}", reference.join(" "));
            }
            wrong += wrong_here;
            total += reference.len();
        }
        let compute = started.elapsed().as_secs_f32();
        eprintln!(
            "RESULT {dir}: WER {:.1}% ({wrong}/{total} words), {:.1}x real time, load {load_ms} ms, {:.0} MB private, {digits} clips with digits",
            100.0 * wrong as f32 / total as f32,
            audio_secs / compute,
            worker.private_mb(),
        );
        drop(worker.input);
        let _ = worker.child.wait();
    }
}
