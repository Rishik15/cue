//! File: tests/corpus_check.rs
//! Purpose: Check a personal dictation corpus before it is benched: enough clips, every WAV 16 kHz mono 16-bit and a sane length, every reference present, protected words listed. Prints counts only, never a transcript.
//! Contents: check_corpus — the ignored test; run with `CUE_CORPUS=<folder> scripts/cue.ps1 lab corpus` (see bench/dictation-corpus/README.md).

#![allow(dead_code)] // stats.rs is included whole; this test needs only words

#[path = "common/stats.rs"]
mod stats;
#[path = "common/wav.rs"]
mod wav;

use stats::words;
use wav::wav_samples;

/// The fewest clips a corpus needs to say anything about names, numbers and technical terms.
const MIN_CLIPS: usize = 30;

#[test]
#[ignore = "checks the user's own corpus; run with CUE_CORPUS set"]
fn check_corpus() {
    let Ok(dir) = std::env::var("CUE_CORPUS") else {
        eprintln!("skipped: CUE_CORPUS not set");
        return;
    };
    let refs = std::fs::read_to_string(format!("{dir}/refs.tsv")).expect("refs.tsv");
    let (mut clips, mut secs, mut protected, mut problems) = (0usize, 0f32, 0usize, Vec::new());
    for (n, line) in refs.lines().filter(|l| !l.trim().is_empty()).enumerate() {
        let mut cols = line.split('\t');
        let id = cols.next().unwrap_or_default();
        if words(cols.next().unwrap_or_default()).is_empty() {
            problems.push(format!("line {}: no reference text", n + 1));
        }
        protected += cols.next().map_or(0, |p| p.split('|').flat_map(words).count());
        match std::fs::read(format!("{dir}/{id}.wav")).map_err(|e| e.to_string()).and_then(|b| wav_samples(&b)) {
            Ok(audio) if (1.0..=60.0).contains(&(audio.len() as f32 / 16_000.0)) => secs += audio.len() as f32 / 16_000.0,
            Ok(_) => problems.push(format!("line {}: {id}.wav is shorter than 1 s or longer than 60 s", n + 1)),
            Err(e) => problems.push(format!("line {}: {id}.wav: {e}", n + 1)),
        }
        clips += 1;
    }
    eprintln!("RESULT corpus clips={clips} audio={secs:.0}s protected_words={protected} problems={}", problems.len());
    problems.iter().for_each(|p| eprintln!("RESULT   {p}"));
    assert!(problems.is_empty(), "fix the files above");
    assert!(clips >= MIN_CLIPS, "need at least {MIN_CLIPS} clips, found {clips}");
    assert!(protected > 0, "list the names, numbers and technical terms as protected words (third column)");
}
