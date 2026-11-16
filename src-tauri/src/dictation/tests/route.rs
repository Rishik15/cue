//! File: dictation/tests/route.rs
//! Purpose: Protect where a finished dictation's result goes: a failure that kept words is rescued to the clipboard and never inserted or submitted, whatever the recording's length.
//! Contents: tests of `route`.

use crate::dictation::{route, Route};
use crate::speech::Failure;

/// A failure that kept words always routes to the rescue, also for a recording too short to keep, and no failure ever routes to an insert.
#[test]
fn a_failure_with_kept_words_is_rescued_and_never_inserted() {
    for keep in [true, false] {
        let failure = Failure { message: "stopped".into(), kept: Some("half a sentence".into()) };
        assert_eq!(route(Err(failure), keep), Route::Rescue { message: "stopped".into(), kept: "half a sentence".into() });
    }
    assert_eq!(route(Err("stopped".into()), true), Route::Report("stopped".into()));
}

/// A finished dictation is inserted, and an accidental tap is dropped.
#[test]
fn a_finished_dictation_is_inserted_and_a_tap_is_dropped() {
    assert_eq!(route(Ok("hello".into()), true), Route::Insert("hello".into()));
    assert_eq!(route(Ok("hello".into()), false), Route::Nothing);
}
