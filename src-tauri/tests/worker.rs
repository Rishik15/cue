//! Purpose: End-to-end check of the speech worker process over its pipe with the real models: load, stream audio in 200 ms pieces the way the
//! recorder does, get the text back, and exit when stdin closes. Skipped unless CUE_TEST_MODEL (a folder holding the model files and
//! silero_vad.onnx) and CUE_TEST_WAV (16 kHz mono 16-bit WAV) are set; `scripts/winsync speech` sets them.

mod common;

use common::{wav_samples, Worker};

#[test]
fn worker_streams_audio_and_cuts_at_pauses() {
    let (Ok(model), Ok(wav)) = (std::env::var("CUE_TEST_MODEL"), std::env::var("CUE_TEST_WAV")) else {
        eprintln!("skipped: CUE_TEST_MODEL / CUE_TEST_WAV not set");
        return;
    };
    let speech = wav_samples(&std::fs::read(wav).expect("wav file"));
    let silence = |secs: f32| vec![0.0f32; (secs * 16_000.0) as usize];
    let family = std::env::var("CUE_TEST_FAMILY").unwrap_or_else(|_| "Transducer".into());
    let mut worker = Worker::start(&model, &family);

    let (plain, partials) = worker.utterance_with_partials(1, &speech);
    assert!(plain.contains("portrait"), "{plain}");
    assert_eq!(partials.join(" "), plain, "the partial results add up to the final text");

    // Silence around and between: two sentences with a pause long enough to cut at, plus pure silence in front.
    let spaced: Vec<f32> = [silence(1.0), speech.clone(), silence(1.5), speech.clone(), silence(0.5)].concat();
    let (both, partials) = worker.utterance_with_partials(2, &spaced);
    assert_eq!(both.matches("portrait").count(), 2, "{both}");
    assert_eq!(partials.len(), 2, "one partial result per sentence: {partials:?}");

    let (quiet, partials) = worker.utterance_with_partials(3, &silence(2.0));
    assert!(quiet.is_empty() && partials.is_empty(), "silence must give no text: {quiet:?}");

    drop(worker.input); // closing stdin is the unload signal
    assert!(worker.child.wait().unwrap().success());
}
