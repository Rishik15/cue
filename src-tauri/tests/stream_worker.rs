//! Purpose: End-to-end check of the streaming speech worker over its pipe with the real model: load, stream a recording in 200 ms pieces the way
//! the recorder does, get clauses back as partial results, and a final text that is exactly their sum. Skipped unless CUE_TEST_STREAM_MODEL
//! (a folder with the four model files and a test_wavs folder) is set; `scripts/winsync stream` sets it.

mod common;

use common::{wav_samples, Worker};

#[test]
fn streaming_worker_hands_out_clauses_that_add_up_to_the_text() {
    let Ok(model) = std::env::var("CUE_TEST_STREAM_MODEL") else {
        eprintln!("skipped: CUE_TEST_STREAM_MODEL not set");
        return;
    };
    let family = std::env::var("CUE_TEST_STREAM_FAMILY").unwrap_or_else(|_| "Streaming".into());
    let mut worker = Worker::start(&model, &family);
    let wavs = std::env::var("CUE_TEST_STREAM_WAVS").unwrap_or_else(|_| "0.wav,1.wav".into());
    for (id, name) in wavs.split(',').enumerate() {
        let speech = wav_samples(&std::fs::read(format!("{model}/test_wavs/{name}")).expect("wav file"));
        let (text, partials) = worker.utterance_with_partials(id as u64 + 1, &speech);
        eprintln!("STREAM {name}: {text:?}\n  clauses: {partials:?}");
        assert!(!text.is_empty(), "{name} gave no text");
        // These two recordings stop right after their last word: it must not come out cut.
        let last = match name {
            "uk.wav" => "країни",
            "vi.wav" => "nước",
            _ => "",
        };
        assert!(text.ends_with(last), "{name}: {text}");
        assert_eq!(partials.join(" "), text, "the clauses add up to the final text");
    }
    drop(worker.input);
    assert!(worker.child.wait().unwrap().success());
}

/// Words that differ between two transcripts of the same audio (substitutions, insertions, deletions), ignoring case and punctuation.
fn word_edits(a: &str, b: &str) -> usize {
    let words = |t: &str| -> Vec<String> {
        t.to_lowercase()
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '\'' { c } else { ' ' })
            .collect::<String>()
            .split_whitespace()
            .map(String::from)
            .collect()
    };
    let (a, b) = (words(a), words(b));
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, x) in a.iter().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, y) in b.iter().enumerate() {
            let above = row[j + 1];
            row[j + 1] = (diagonal + usize::from(x != y)).min(row[j] + 1).min(above + 1);
            diagonal = above;
        }
    }
    row[b.len()]
}

/// The streaming model against the offline one on the same speech: they must agree on nearly every word. Needs CUE_TEST_OFFLINE_MODEL too.
#[test]
fn streaming_agrees_with_offline_on_the_same_speech() {
    let (Ok(stream), Ok(offline)) = (std::env::var("CUE_TEST_STREAM_MODEL"), std::env::var("CUE_TEST_OFFLINE_MODEL")) else { return };
    let (mut live, mut batch) = (Worker::start(&stream, "Streaming"), Worker::start(&offline, "Transducer"));
    let (mut words, mut edits) = (0, 0);
    for (id, name) in ["0.wav", "1.wav"].iter().enumerate() {
        let speech = wav_samples(&std::fs::read(format!("{stream}/test_wavs/{name}")).expect("wav file"));
        let (a, _) = live.utterance_with_partials(id as u64 + 1, &speech);
        let (b, _) = batch.utterance_with_partials(id as u64 + 1, &speech);
        eprintln!("PARITY {name}\n  stream:  {a}\n  offline: {b}");
        (words, edits) = (words + b.split_whitespace().count(), edits + word_edits(&a, &b));
    }
    eprintln!("PARITY {edits} differing words of {words}");
    assert!(edits * 10 <= words, "the streaming text differs from the offline text on more than 10 % of the words");
}

/// One long recording: looped speech with pauses, fed as fast as the worker takes it, as a single stream. The worker's private memory must stay
/// flat after the first minute and the text must keep coming. 10 minutes of audio; the 60 s rounds are only for sampling the memory.
#[test]
#[ignore = "long; run with scripts/winsync streamlong"]
fn one_long_stream_does_not_grow() {
    let Ok(model) = std::env::var("CUE_TEST_STREAM_MODEL") else { return };
    const MINUTES: usize = 10;
    let wav = |name: &str| wav_samples(&std::fs::read(format!("{model}/test_wavs/{name}")).expect("wav file"));
    let one_round: Vec<f32> = [wav("0.wav"), vec![0.0; 24_000], wav("1.wav"), vec![0.0; 40_000]].concat();
    let mut worker = Worker::start(&model, "Streaming");
    worker.begin(1);
    let mut sizes = Vec::new();
    for minute in 0..MINUTES {
        let mut audio = Vec::new();
        while audio.len() < 16_000 * 60 {
            audio.extend_from_slice(&one_round);
        }
        worker.feed(&audio);
        std::thread::sleep(std::time::Duration::from_secs(3)); // let the worker catch up before looking at its memory
        sizes.push(worker.private_mb());
        eprintln!("MINUTE {} private {:.0} MB", minute + 1, sizes[minute]);
    }
    let (text, partials) = worker.end(1, (MINUTES * 60) as f32);
    eprintln!("LONG {} words in {} clauses", text.split_whitespace().count(), partials.len());
    assert!(text.split_whitespace().count() > MINUTES * 150, "the text kept coming for the whole recording");
    assert!(sizes[MINUTES - 1] < sizes[1] + 150.0, "memory grew from {:.0} MB to {:.0} MB", sizes[1], sizes[MINUTES - 1]);
}
