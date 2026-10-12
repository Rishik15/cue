//! Purpose: The pure audio arithmetic of the speech worker: which part of the recording to decode for a segment, how much audio to keep, and where
//! to cut speech that never pauses. No model or process, so each piece is tested on plain arrays.
//! Contents: padded — a segment's range with margin; trim_point — start of the audio worth keeping; quietest — the best place to cut long speech;
//! is_loud — whether audio the detector ignored was really sound.

use crate::audio::RATE;

/// Speech with no pause is cut by us after this long, at its quietest moment near the end (see `quietest`). The detector's own forced cut
/// is only a backstop behind it, because it cuts wherever the clock says, often inside a word.
pub const CUT_SECS: usize = 10;
pub const CUT_LOOK_SECS: usize = 2;
/// The worker keeps this much of the latest audio (the recording can last an hour). Anything older is decoded or was silence, because
/// speech is cut into segments at least this often.
const KEEP: usize = RATE as usize * 40;

/// The detector reports speech a little late and cuts it right at the last voiced frame, which clips the first and last sounds ("I begged"
/// became "begged"). Each segment is therefore re-cut from the recorded audio with some margin.
const PAD_BEFORE: usize = RATE as usize * 3 / 10;
const PAD_AFTER: usize = RATE as usize / 5;
/// Audio this loud with no speech found by the detector is decoded whole rather than dropped.
const LOUD: f32 = 0.03;
/// How many samples must be that loud: a click or a cough has a few hundred, speech thousands.
const MIN_LOUD: usize = RATE as usize / 10;

/// The range of the recording to decode for a segment of `n` samples starting at `start`: padded, and clamped so that it neither overlaps
/// what was already decoded nor runs past the audio received so far.
pub fn padded(start: usize, n: usize, done_to: usize, len: usize) -> (usize, usize) {
    let from = start.saturating_sub(PAD_BEFORE).max(done_to).min(len);
    let to = (start + n + PAD_AFTER).min(len).max(from);
    (from, to)
}

/// Where the kept audio starts once `total` samples have arrived: never earlier than it already does.
pub fn trim_point(total: usize, base: usize) -> usize {
    total.saturating_sub(KEEP).max(base)
}

/// The sample in `samples` at the centre of its quietest 100 ms (the latest one on a tie): the gap between two words is the safest place to cut.
/// Also whether that gap is a real pause (a fifth of the speech level or less) as opposed to a dip inside running speech.
pub fn quietest(samples: &[f32]) -> (usize, bool) {
    const WINDOW: usize = RATE as usize / 10;
    const HOP: usize = RATE as usize / 100;
    if samples.len() <= WINDOW {
        return (samples.len(), false);
    }
    let mut energy = vec![0.0f64; samples.len() + 1]; // running sum of squares: any window's energy is two lookups
    for (i, s) in samples.iter().enumerate() {
        energy[i + 1] = energy[i] + (*s as f64) * (*s as f64);
    }
    let window = |a: &usize| energy[a + WINDOW] - energy[*a];
    let starts = (0..=samples.len() - WINDOW).step_by(HOP);
    let Some(best) = starts.min_by(|a, b| window(a).total_cmp(&window(b)).then(b.cmp(a))) else { return (samples.len(), false) };
    let (quiet, overall) = (window(&best) / WINDOW as f64, energy[samples.len()] / samples.len() as f64);
    (best + WINDOW / 2, quiet * 25.0 < overall) // energy is rms squared: a fifth of the level is 1/25 of the energy
}

pub fn is_loud(samples: &[f32]) -> bool {
    samples.iter().filter(|s| s.abs() > LOUD).count() >= MIN_LOUD
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segments_are_padded_without_overlap_or_overrun() {
        let r = crate::audio::RATE as usize;
        assert_eq!(padded(r, r, 0, 10 * r), (r - r * 3 / 10, 2 * r + r / 5)); // margin on both sides
        assert_eq!(padded(1000, 5000, 0, 90_000), (0, 6000 + r / 5)); // cannot start before the recording
        assert_eq!(padded(r, r, r + r / 2, 10 * r).0, r + r / 2); // does not repeat audio already decoded
        assert_eq!(padded(r, r, 0, 2 * r + 100).1, 2 * r + 100); // does not run past what has arrived
    }

    #[test]
    fn long_speech_is_cut_in_its_quietest_gap() {
        let r = crate::audio::RATE as usize;
        let mut speech = vec![0.3f32; 2 * r];
        speech[r..r + r / 5].fill(0.0); // a 200 ms gap one second in
        let (cut, pause) = quietest(&speech);
        assert!((r..r + r / 5).contains(&cut) && pause, "{cut} {pause}");
        assert_eq!(quietest(&[0.1; 100]), (100, false)); // too short to look for a gap: the end
        assert!(!quietest(&vec![0.3; 2 * r]).1); // steady sound has no pause in it
        let mut dip = vec![0.3f32; 2 * r];
        dip[r..r + r / 10].fill(0.2);
        assert!(!quietest(&dip).1); // a dip is not a pause
    }

    #[test]
    fn only_the_latest_audio_is_kept() {
        let r = crate::audio::RATE as usize;
        assert_eq!(trim_point(10 * r, 0), 0); // short recordings keep everything
        assert_eq!(trim_point(100 * r, 0), 60 * r); // a long one keeps the last 40 s
        assert_eq!(trim_point(100 * r, 70 * r), 70 * r); // never moves back
    }

    #[test]
    fn loud_audio_is_recognised_quiet_and_tiny_is_not() {
        assert!(is_loud(&vec![0.1; 8_000]));
        assert!(!is_loud(&vec![0.001; 8_000]));
        assert!(!is_loud(&[0.5; 100])); // a click
    }
}
