//! File: status/tests.rs
//! Purpose: Protect the tooltip state: the latest wanted text wins and every change is numbered, so a slower writer notices it was overtaken and the tooltip ends on the newest text whatever the order of the threads.
//! Contents: a test of `want`, `wanted` and `converge`.

use super::{converge, want, wanted, IDLE};

/// A report overtaken by a clear while it is being applied is applied again as the clear, so the tooltip ends on the idle text and not on the stale problem; with nothing overtaking, the text is applied once. One test, because the state is global.
#[test]
fn the_tooltip_ends_on_the_newest_text() {
    want("Cue: a problem");
    let mut applied = Vec::new();
    let mut cleared = false;
    converge(|text| {
        applied.push(text.to_string());
        if !cleared {
            cleared = true;
            want("");
        }
    });
    assert_eq!(applied, ["Cue: a problem", IDLE]);
    assert!(wanted().text.is_empty());

    want("Cue: another");
    let mut calls = 0;
    converge(|_| calls += 1);
    assert_eq!(calls, 1);
    want("");
}
