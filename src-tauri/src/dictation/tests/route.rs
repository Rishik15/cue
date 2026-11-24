//! File: dictation/tests/route.rs
//! Purpose: Protect where a finished dictation's result goes: a failure that kept words is rescued to the clipboard and never inserted or submitted, whatever the recording's length, checked on the delivery actually chosen.
//! Contents: tests of `route` and `dispatch`.

use std::cell::RefCell;
use std::rc::Rc;

use crate::audio::Loss;
use crate::dictation::routing::{dispatch, route, Deliver, Route};
use crate::speech::Failure;

/// A delivery that only records which way a result left (insertion is the only way into the rules and the submit key).
struct Spy(Rc<RefCell<Vec<String>>>);

impl Deliver for Spy {
    fn insert(self, text: String) {
        self.0.borrow_mut().push(format!("insert {text}"));
    }
    fn rescue(self, message: &str, kept: &str) {
        self.0.borrow_mut().push(format!("rescue {message}|{kept}"));
    }
    fn report(self, message: &str) {
        self.0.borrow_mut().push(format!("report {message}"));
    }
}

/// What a result did, step by step. Inputs: the engine's result and whether the recording is long enough to keep.
fn delivered(result: Result<String, Failure>, keep: bool) -> Vec<String> {
    let log = Rc::default();
    dispatch(route(result, keep, None), Spy(Rc::clone(&log)));
    log.take()
}

/// A failure that kept words reaches only the clipboard rescue, with the raw kept text, also for a recording too short to keep; it is never inserted (so never run through the rules or submitted).
#[test]
fn a_failure_with_kept_words_is_rescued_and_never_inserted() {
    for keep in [true, false] {
        let failure = Failure { message: "stopped".into(), kept: Some("half a sentence".into()) };
        assert_eq!(delivered(Err(failure), keep), ["rescue stopped|half a sentence"]);
    }
    assert_eq!(delivered(Err("stopped".into()), true), ["report stopped"]);
}

/// A finished dictation is inserted, and an accidental tap delivers nothing.
#[test]
fn a_finished_dictation_is_inserted_and_a_tap_is_dropped() {
    assert_eq!(delivered(Ok("hello".into()), true), ["insert hello"]);
    assert!(delivered(Ok("hello".into()), false).is_empty());
    assert_eq!(route(Ok("hello".into()), true, None), Route::Insert("hello".into()));
}

/// Words from a lossy recording (broken stream, overflow) go to the clipboard rescue; blank text, a tap and a lossless recording do not.
#[test]
fn a_lossy_recording_with_words_is_rescued() {
    let heard = |loss| route(Ok("hello".into()), true, loss);
    assert_eq!(heard(Some(Loss::Broken)), Route::Rescue { message: "The microphone stopped working.".into(), kept: "hello".into() });
    assert_eq!(heard(Some(Loss::Overflow)), Route::Rescue { message: "Part of the recording was lost.".into(), kept: "hello".into() });
    assert_eq!(route(Ok(" ".into()), true, Some(Loss::Broken)), Route::Insert(" ".into()));
    assert_eq!(heard(None), Route::Insert("hello".into()));
    assert_eq!(route(Ok("hello".into()), false, Some(Loss::Broken)), Route::Nothing);
}
