//! File: tests/speech_stats.rs
//! Purpose: Tests of the speech benches' arithmetic (percentile, word errors, clip plan), so a number in the bench table can be trusted.
//! Contents: tests of common/stats.rs, which this file includes by path (it needs no model, no worker and no Windows).

#[path = "common/stats.rs"]
mod stats;
#[path = "common/wav.rs"]
mod wav;

use stats::{cold_count, cut_to, errors, lost_words, percentile, plan_clips, words, Errors};
use wav::wav_samples;

/// Splits a sentence into the words the bench compares.
fn w(text: &str) -> Vec<String> {
    words(text)
}

/// The percentile is the nearest rank, for unsorted, single and empty input.
#[test]
fn percentiles_use_the_nearest_rank() {
    let v: Vec<f64> = (1..=100).map(f64::from).collect();
    assert_eq!(percentile(&v, 50.0), 50.0);
    assert_eq!(percentile(&v, 95.0), 95.0);
    assert_eq!(percentile(&[3.0, 1.0, 2.0], 50.0), 2.0);
    assert_eq!(percentile(&[7.0], 95.0), 7.0);
    assert_eq!(percentile(&[], 50.0), 0.0);
}

/// A word missing from the transcript is one deletion, not a substitution and an insertion.
#[test]
fn a_dropped_word_counts_as_a_deletion_not_a_substitution() {
    let reference = w("Send it to Bob at 4 pm");
    let clipped = errors(&reference, &w("send it to at 4 pm"));
    assert_eq!(clipped, Errors { substitutions: 0, insertions: 0, deletions: 1 });
    let swapped = errors(&reference, &w("send it to Rob at 4 pm"));
    assert_eq!(swapped, Errors { substitutions: 1, insertions: 0, deletions: 0 });
    let extra = errors(&w("a b"), &w("a x b"));
    assert_eq!(extra, Errors { substitutions: 0, insertions: 1, deletions: 0 });
    assert_eq!(errors(&w("same words"), &w("Same, words!")).total(), 0);
}

/// An empty transcript deletes every word of the reference.
#[test]
fn an_empty_transcript_deletes_every_word() {
    assert_eq!(errors(&w("one two three"), &[]).deletions, 3);
    assert_eq!(errors(&[], &w("one two")).insertions, 2);
}

/// Clips are planned to reach their target length, the same every run, wrapping round the set.
#[test]
fn clips_reach_their_target_deterministically_and_wrap() {
    let lengths = [2.0, 4.0, 5.0];
    let plan = plan_clips(&lengths, 10.0, 3);
    assert_eq!(plan, vec![vec![0, 1, 2], vec![0, 1, 2], vec![0, 1, 2]]);
    assert_eq!(plan, plan_clips(&lengths, 10.0, 3));
    for clip in plan_clips(&lengths, 3.0, 5) {
        assert!(clip.iter().map(|i| lengths[*i]).sum::<f32>() >= 3.0);
    }
    assert!(plan_clips(&[], 3.0, 5).is_empty());
    assert!(plan_clips(&[0.0], 3.0, 5).is_empty());
}

/// A protected word counts as lost once per occurrence the transcript lacks.
#[test]
fn a_protected_word_is_lost_once_per_missing_occurrence() {
    assert_eq!(lost_words(&w("bob 4500"), &w("send it to bob for 4500")), 0);
    assert_eq!(lost_words(&w("bob 4500"), &w("send it to for 45")), 2);
    assert_eq!(lost_words(&w("bob bob"), &w("bob is here")), 1);
    assert_eq!(lost_words(&[], &w("anything")), 0);
}

/// A WAV file with the given `fmt ` fields and chunks after it, each chunk as (id, body).
fn wav(tag: u16, channels: u16, rate: u32, bits: u16, chunks: &[(&[u8; 4], Vec<u8>)]) -> Vec<u8> {
    let mut fmt = [tag.to_le_bytes().to_vec(), channels.to_le_bytes().to_vec(), rate.to_le_bytes().to_vec()].concat();
    fmt.extend_from_slice(&(rate * u32::from(channels) * u32::from(bits) / 8).to_le_bytes());
    fmt.extend_from_slice(&(channels * bits / 8).to_le_bytes());
    fmt.extend_from_slice(&bits.to_le_bytes());
    let mut out = b"RIFF\0\0\0\0WAVE".to_vec();
    for (id, body) in std::iter::once((b"fmt ", fmt)).chain(chunks.iter().cloned()) {
        out.extend_from_slice(id);
        out.extend_from_slice(&(body.len() as u32).to_le_bytes());
        out.extend_from_slice(&body);
        out.extend(std::iter::repeat_n(0, body.len() & 1));
    }
    let riff = (out.len() - 8) as u32;
    out[4..8].copy_from_slice(&riff.to_le_bytes());
    out
}

/// Two samples (16384 and -32768) as bytes.
fn pcm() -> Vec<u8> {
    [16384i16.to_le_bytes(), (-32768i16).to_le_bytes()].concat()
}

/// The WAV reader takes samples from the data chunk only, skipping other chunks.
#[test]
fn wav_reader_decodes_only_the_data_chunk() {
    let plain = wav(1, 1, 16_000, 16, &[(b"data", pcm())]);
    assert_eq!(wav_samples(&plain).unwrap(), vec![0.5, -1.0]);
    let trailing = wav(1, 1, 16_000, 16, &[(b"data", pcm()), (b"LIST", b"INFOdata junk".to_vec())]);
    assert_eq!(wav_samples(&trailing).unwrap().len(), 2);
    let leading = wav(1, 1, 16_000, 16, &[(b"LIST", b"has data inside".to_vec()), (b"data", pcm())]);
    assert_eq!(wav_samples(&leading).unwrap(), vec![0.5, -1.0]);
}

/// The WAV reader refuses a wrong format and damaged files.
#[test]
fn wav_reader_rejects_wrong_formats_and_damage() {
    for bad in [
        wav(1, 2, 16_000, 16, &[(b"data", pcm())]),
        wav(1, 1, 48_000, 16, &[(b"data", pcm())]),
        wav(3, 1, 16_000, 32, &[(b"data", pcm())]),
        wav(1, 1, 16_000, 8, &[(b"data", pcm())]),
    ] {
        assert!(wav_samples(&bad).unwrap_err().contains("16 kHz mono 16-bit"));
    }
    let mut cut = wav(1, 1, 16_000, 16, &[(b"data", pcm())]);
    cut.truncate(cut.len() - 1);
    assert!(wav_samples(&cut).is_err());
    assert!(wav_samples(b"not a wav at all").is_err());
    assert!(wav_samples(&wav(1, 1, 16_000, 16, &[])).is_err());
}

/// The WAV reader refuses PCM data that is not whole samples.
#[test]
fn wav_reader_rejects_malformed_pcm() {
    let odd = wav(1, 1, 16_000, 16, &[(b"data", vec![1, 2, 3])]);
    assert!(wav_samples(&odd).unwrap_err().contains("partial sample"));
    let mut too_big = wav(1, 1, 16_000, 16, &[(b"data", pcm())]);
    too_big[4..8].copy_from_slice(&1_000_000u32.to_le_bytes());
    assert!(wav_samples(&too_big).unwrap_err().contains("RIFF size"));
    let mut bad_rate = wav(1, 1, 16_000, 16, &[(b"data", pcm())]);
    bad_rate[28..32].copy_from_slice(&1u32.to_le_bytes());
    assert!(wav_samples(&bad_rate).unwrap_err().contains("inconsistent"));
    let mut bad_align = wav(1, 1, 16_000, 16, &[(b"data", pcm())]);
    bad_align[32] = 4;
    assert!(wav_samples(&bad_align).unwrap_err().contains("inconsistent"));
    let mut trailing = wav(1, 1, 16_000, 16, &[(b"data", pcm())]);
    trailing.extend_from_slice(b"junk after RIFF");
    assert_eq!(wav_samples(&trailing).unwrap().len(), 2);
}

/// A clip is cut to exactly its bucket length.
#[test]
fn a_clip_is_cut_to_exactly_its_bucket_length() {
    let audio = vec![0.0; 50_000];
    assert_eq!(cut_to(&audio, 3.0, 16_000.0).len(), 48_000);
    assert_eq!(cut_to(&audio, 10.0, 16_000.0).len(), 50_000);
}

/// The cold-worker count setting must be a positive number.
#[test]
fn the_cold_worker_count_must_be_a_positive_number() {
    assert_eq!(cold_count("10"), Ok(10));
    assert_eq!(cold_count(" 1 "), Ok(1));
    assert!(cold_count("0").is_err());
    assert!(cold_count("x").is_err());
    assert!(cold_count("-2").is_err());
}
