//! File: dictation/tests/abandon.rs
//! Purpose: Protect that a recording's cleanup runs only for the one that ended it: an ending that found nothing (another recording owns the phase) touches neither the microphone, the speech engine nor the stop clock.
//! Contents: tests of `abandon`.

use crate::dictation::recording::Phase;
use crate::dictation::{abandon, LAST_STOP};
use crate::platform::Target;

/// An abandon that was handed no ending does nothing, and one that holds an ending keeps the phase busy until the cleanup is over (a press during it is dropped), then frees it.
#[test]
fn cleanup_runs_only_for_the_recording_that_was_ended() {
    *LAST_STOP.lock().unwrap() = None;
    abandon(None);
    assert!(LAST_STOP.lock().unwrap().is_none(), "no cleanup without ownership");
    let phase = Phase::new();
    assert!(phase.begin(Target::default));
    phase.publish(5);
    assert!(phase.end_for(6).is_none(), "a late failure of another recording owns nothing");
    let held = phase.end_for(5);
    assert!(held.is_some() && !phase.begin(Target::default), "busy while the hold lasts");
    drop(held);
    assert!(phase.begin(Target::default));
}
