//! File: speech/keep.rs
//! Purpose: The one rule both audio buffers (`Take` in the app, `Engine` in the worker) trim by: always keep the latest `keep` samples, but move the
//! buffer only once `slack` more have piled up, so a long recording is not shifted on every chunk.
//! Contents: surplus — how many of the oldest samples to drop now.

/// How many of the oldest samples to drop now. Inputs: the buffer length, the samples that must stay, the extra allowed before trimming.
/// Returns 0 while `len <= keep + slack`, else `len - keep`.
pub(super) fn surplus(len: usize, keep: usize, slack: usize) -> usize {
    if len <= keep + slack {
        0
    } else {
        len - keep
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// Nothing is dropped up to the slack; past it, everything over `keep` goes.
    #[test]
    fn waits_for_the_slack() {
        assert_eq!(surplus(100, 100, 10), 0);
        assert_eq!(surplus(110, 100, 10), 0);
        assert_eq!(surplus(111, 100, 10), 11);
        assert_eq!(surplus(5, 100, 10), 0);
    }

    proptest! {
        /// Whatever the chunk sizes, the buffer always holds the latest `keep` samples, stays within `keep + slack` after a step, and loses or repeats nothing.
        #[test]
        fn keeps_the_latest_samples(chunks in prop::collection::vec(0usize..500, 0..200), keep in 1usize..300, slack in 0usize..100) {
            let (mut base, mut len, mut total) = (0usize, 0usize, 0usize);
            for chunk in chunks {
                len += chunk;
                total += chunk;
                let drop = surplus(len, keep, slack);
                len -= drop;
                base += drop;
                prop_assert!(len <= keep + slack);
                prop_assert!(base + len == total);
                prop_assert!(base <= total.saturating_sub(keep));
            }
        }
    }
}
