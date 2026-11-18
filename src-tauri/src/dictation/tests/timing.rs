//! File: dictation/tests/timing.rs
//! Purpose: Protect the one log line that shows where the wait between letting go of the key and the text went: stage names and milliseconds only, never what was said.
//! Contents: tests of `timing::line`.

use crate::dictation::timing::line;

/// The line names every stage with its milliseconds since the release, in the order they happened, plus the length spoken and the thread count.
#[test]
fn the_line_lists_each_stage_since_the_release() {
    let marks = [("regrip", 201), ("tail", 214), ("final", 330), ("clean", 331), ("delivered", 380)];
    assert_eq!(
        line(4200, 2, &marks),
        "dictation: 4200 ms spoken, 2 threads, ms after release: regrip 201, tail 214, final 330, clean 331, delivered 380"
    );
}

/// A dictation that stopped before some stage still prints the ones that happened.
#[test]
fn a_short_list_prints_as_far_as_it_got() {
    assert_eq!(line(0, 1, &[("regrip", 200)]), "dictation: 0 ms spoken, 1 threads, ms after release: regrip 200");
}
