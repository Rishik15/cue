//! File: tests/common/stats.rs
//! Purpose: The pure arithmetic of the speech benches: percentiles, word-level error counts (with deletions apart, they are the clipped words), and the fixed clip plan.
//! Contents: percentile; lost_words — protected words missing from a text; words; Errors / errors — substitutions, insertions, deletions; plan_clips — which utterances make each fixed-length clip.

/// The p-th percentile (0 to 100, nearest rank) of the values. Inputs: the values in any order and p. Returns 0 for an empty list.
pub fn percentile(values: &[f64], p: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let rank = ((p / 100.0) * sorted.len() as f64).ceil() as usize;
    sorted[rank.clamp(1, sorted.len()) - 1]
}

/// Lower case words only (letters, digits, apostrophes): the usual minimal normalisation before comparing. Input: text. Returns the words.
pub fn words(text: &str) -> Vec<String> {
    let spaced: String = text.to_lowercase().chars().map(|c| if c.is_alphanumeric() || c == '\'' { c } else { ' ' }).collect();
    spaced.split_whitespace().map(String::from).collect()
}

/// Word-level differences between a reference and what was heard.
#[derive(Debug, PartialEq, Eq, Clone, Copy, Default)]
pub struct Errors {
    pub substitutions: usize,
    pub insertions: usize,
    /// Reference words that are missing from the text: what a clipped pause or a cut name shows up as.
    pub deletions: usize,
}

impl Errors {
    /// All differences added together.
    pub fn total(&self) -> usize {
        self.substitutions + self.insertions + self.deletions
    }
}

/// The least-effort word alignment of `heard` to `reference`. Inputs: both word lists. Returns the counts of each kind of difference.
pub fn errors(reference: &[String], heard: &[String]) -> Errors {
    let (n, m) = (reference.len(), heard.len());
    let mut cost = vec![vec![0usize; m + 1]; n + 1];
    for (i, row) in cost.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, cell) in cost[0].iter_mut().enumerate() {
        *cell = j;
    }
    for i in 1..=n {
        for j in 1..=m {
            let same = usize::from(reference[i - 1] != heard[j - 1]);
            cost[i][j] = (cost[i - 1][j - 1] + same).min(cost[i - 1][j] + 1).min(cost[i][j - 1] + 1);
        }
    }
    let (mut i, mut j, mut out) = (n, m, Errors::default());
    while i > 0 || j > 0 {
        if i > 0 && j > 0 && cost[i][j] == cost[i - 1][j - 1] + usize::from(reference[i - 1] != heard[j - 1]) {
            out.substitutions += usize::from(reference[i - 1] != heard[j - 1]);
            (i, j) = (i - 1, j - 1);
        } else if i > 0 && cost[i][j] == cost[i - 1][j] + 1 {
            out.deletions += 1;
            i -= 1;
        } else {
            out.insertions += 1;
            j -= 1;
        }
    }
    out
}

/// Which utterances (indices into `lengths`, their durations in seconds) are joined into each fixed-length clip. Inputs: the utterance
/// durations, the target length and the clip count. Returns, per clip, the utterance indices; each clip takes utterances in turn from where the
/// last one stopped (wrapping) until it is at least `target` long, so the same inputs always give the same clips. Empty when there are no utterances.
pub fn plan_clips(lengths: &[f32], target: f32, clips: usize) -> Vec<Vec<usize>> {
    if lengths.is_empty() || lengths.iter().all(|l| *l <= 0.0) {
        return Vec::new();
    }
    let mut next = 0;
    (0..clips)
        .map(|_| {
            let (mut picked, mut secs) = (Vec::new(), 0.0);
            while secs < target {
                picked.push(next);
                secs += lengths[next];
                next = (next + 1) % lengths.len();
            }
            picked
        })
        .collect()
}

/// How many of the protected words (names, numbers: already in `words` form) are missing from what was heard, each protected word counted once per
/// occurrence. Inputs: the protected words and the heard words. Returns the count of those lost.
pub fn lost_words(protected: &[String], heard: &[String]) -> usize {
    let mut left: Vec<&String> = heard.iter().collect();
    protected
        .iter()
        .filter(|word| match left.iter().position(|h| h == word) {
            Some(at) => {
                left.swap_remove(at);
                false
            }
            None => true,
        })
        .count()
}
