//! File: tests/speech_stats.rs
//! Purpose: Tests of the speech benches' arithmetic (percentile, word errors, clip plan), so a number in the bench table can be trusted.
//! Contents: tests of common/stats.rs, which this file includes by path (it needs no model, no worker and no Windows).

#[path = "common/stats.rs"]
mod stats;

use stats::{errors, lost_words, percentile, plan_clips, words, Errors};

/// Splits a sentence into the words the bench compares.
fn w(text: &str) -> Vec<String> {
    words(text)
}

#[test]
fn percentiles_use_the_nearest_rank() {
    let v: Vec<f64> = (1..=100).map(f64::from).collect();
    assert_eq!(percentile(&v, 50.0), 50.0);
    assert_eq!(percentile(&v, 95.0), 95.0);
    assert_eq!(percentile(&[3.0, 1.0, 2.0], 50.0), 2.0);
    assert_eq!(percentile(&[7.0], 95.0), 7.0);
    assert_eq!(percentile(&[], 50.0), 0.0);
}

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

#[test]
fn an_empty_transcript_deletes_every_word() {
    assert_eq!(errors(&w("one two three"), &[]).deletions, 3);
    assert_eq!(errors(&[], &w("one two")).insertions, 2);
}

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

#[test]
fn a_protected_word_is_lost_once_per_missing_occurrence() {
    assert_eq!(lost_words(&w("bob 4500"), &w("send it to bob for 4500")), 0);
    assert_eq!(lost_words(&w("bob 4500"), &w("send it to for 45")), 2);
    assert_eq!(lost_words(&w("bob bob"), &w("bob is here")), 1);
    assert_eq!(lost_words(&[], &w("anything")), 0);
}
