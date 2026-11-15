//! File: dictation/tests/recording.rs
//! Purpose: Protect the start and end of a recording: ending one lowers the flag and hands over the field in one step, only one of two stops gets it, and a recording begun again afterwards has its own field and flag.
//! Contents: one test over `begin_recording` and `end_recording`.

use std::sync::atomic::Ordering;

use crate::dictation::{begin_recording, end_recording, RECORDING, TARGET};

/// Begin raises the flag and holds a field; end returns it once and lowers the flag; a second end finds nothing (Escape and a stop never both deliver); a new begin starts clean.
#[test]
fn a_recording_ends_once_and_a_new_one_starts_clean() {
    begin_recording();
    assert!(RECORDING.load(Ordering::Acquire));
    assert!(TARGET.lock().unwrap().is_some());
    assert!(end_recording().is_some());
    assert!(!RECORDING.load(Ordering::Acquire));
    assert!(TARGET.lock().unwrap().is_none());
    assert!(end_recording().is_none());
    begin_recording();
    assert!(RECORDING.load(Ordering::Acquire));
    assert!(end_recording().is_some());
}
