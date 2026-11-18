//! File: dictation/tests/flow.rs
//! Purpose: Protect who owns a dictation and how it ends: the reducer in `flow` fed the event sequences that raced in the old code (a failure before the id is published, a cap against a stop, Escape against a stop, a late callback after a new start) and a failure after partial text.
//! Contents: tests of `State::apply` and `finish`.

use crate::dictation::flow::{finish, Effect, Event, State};
use crate::dictation::routing::Route;
use crate::platform::Target;
use crate::speech::Failure;

/// A field with a window number, to tell two recordings apart.
fn field(window: isize) -> Target {
    Target { window, ..Target::default() }
}

/// A state with recording `id` running in window `window`.
fn recording(id: u64, window: isize) -> State {
    let mut state = State::new();
    assert_eq!(state.apply(Event::Start(field(window))), [Effect::BeginSpeech]);
    assert_eq!(state.apply(Event::Published(id)), [Effect::OpenMic(id)]);
    state
}

/// A confirmed stop of recording `id`.
fn stop(id: u64) -> Event {
    Event::Stop { id, confirmed: true }
}

/// A press while a recording runs or is being ended starts nothing; once the stop is issued the next press starts.
#[test]
fn a_press_during_a_recording_or_its_ending_is_dropped() {
    let mut state = recording(1, 7);
    assert!(state.apply(Event::Start(field(8))).is_empty());
    assert_eq!(state.apply(stop(1)), [Effect::StopMic { id: 1, target: field(7) }]);
    assert!(state.apply(Event::Start(field(8))).is_empty(), "still ending");
    state.apply(Event::StopIssued);
    assert_eq!(state.apply(Event::Start(field(8))), [Effect::BeginSpeech]);
}

/// A release that did not stay up through the regrip window does not end the recording.
#[test]
fn an_unconfirmed_stop_leaves_the_recording_running() {
    let mut state = recording(1, 7);
    assert!(state.apply(Event::Stop { id: 1, confirmed: false }).is_empty());
    assert!(state.is_recording());
}

/// The engine fails before the recording has published its id: nothing happens yet, and publishing the same id opens the microphone and ends it at once, forced past the regrip check.
#[test]
fn a_failure_before_the_id_is_published_ends_the_recording_when_it_is() {
    let mut state = State::new();
    state.apply(Event::Start(field(7)));
    assert!(state.apply(Event::Failed(5)).is_empty());
    assert_eq!(state.apply(Event::Published(5)), [Effect::OpenMic(5), Effect::StopSoon(5)]);
    assert_eq!(state.apply(Event::Stop { id: 5, confirmed: false }), [Effect::StopMic { id: 5, target: field(7) }]);
}

/// A failure of another recording's engine, before or after publication, ends nothing.
#[test]
fn a_failure_of_another_recording_is_ignored() {
    let mut state = State::new();
    state.apply(Event::Start(field(7)));
    state.apply(Event::Failed(4));
    assert_eq!(state.apply(Event::Published(5)), [Effect::OpenMic(5)]);
    assert!(state.apply(Event::Failed(4)).is_empty());
    assert_eq!(state.apply(Event::Failed(5)), [Effect::StopSoon(5)]);
}

/// The cap and a stop on the same recording: the cap reports once and asks for a stop; of the two stops that follow, one takes the field and the other finds nothing.
#[test]
fn a_cap_and_a_stop_end_the_recording_once() {
    let mut state = recording(1, 7);
    assert_eq!(state.apply(Event::Capped(1)), [Effect::Capped, Effect::StopSoon(1)]);
    assert_eq!(state.apply(stop(1)), [Effect::StopMic { id: 1, target: field(7) }]);
    assert!(state.apply(stop(1)).is_empty());
    assert!(state.apply(Event::Capped(1)).is_empty(), "no second message once it is ending");
}

/// Escape and a stop on the same recording: whichever comes first owns the ending, the other does nothing.
#[test]
fn escape_and_a_stop_end_the_recording_once() {
    let mut state = recording(1, 7);
    assert_eq!(state.apply(Event::Abandon(None)), [Effect::Discard]);
    assert!(state.apply(stop(1)).is_empty());
    let mut state = recording(2, 7);
    assert_eq!(state.apply(stop(2)).len(), 1);
    assert!(state.apply(Event::Abandon(None)).is_empty());
}

/// Callbacks of recording 1 that arrive after recording 2 started change nothing, also while 2 has not published its id yet.
#[test]
fn a_late_callback_of_an_earlier_recording_leaves_the_next_one_alone() {
    let mut state = recording(1, 7);
    state.apply(stop(1));
    state.apply(Event::StopIssued);
    state.apply(Event::Start(field(8)));
    let late = |state: &mut State| {
        [Event::Capped(1), Event::Failed(1), Event::Abandon(Some(1)), stop(1)].into_iter().all(|e| state.apply(e).is_empty())
    };
    assert!(late(&mut state), "before the new id");
    assert_eq!(state.apply(Event::Published(2)), [Effect::OpenMic(2)]);
    assert!(late(&mut state), "after the new id");
    assert!(state.is_recording());
    assert_eq!(state.apply(stop(2)), [Effect::StopMic { id: 2, target: field(8) }]);
}

/// A caption belongs to the recording that published the id, also while it is being ended, and to no other.
#[test]
fn a_caption_shows_only_for_its_own_recording() {
    let mut state = recording(1, 7);
    assert!(state.is_current(1) && !state.is_current(2));
    state.apply(stop(1));
    assert!(state.is_current(1), "the last words still show while the engine finishes");
}

/// An ending that names a recording ends only that one; Escape ends whichever runs.
#[test]
fn an_abandon_names_its_recording() {
    let mut state = recording(1, 7);
    assert!(state.apply(Event::Abandon(Some(2))).is_empty());
    assert_eq!(state.apply(Event::Abandon(Some(1))), [Effect::Discard]);
}

/// A failure after partial text asks for the clipboard rescue with the raw kept words, for a long recording and for an accidental tap alike, and never for an insertion.
#[test]
fn a_failure_after_partial_text_requests_the_rescue() {
    for keep in [true, false] {
        let failure = Failure { message: "stopped".into(), kept: Some("half a sentence".into()) };
        let done = finish(4, keep, Err(failure));
        assert_eq!(done.route, Route::Rescue { message: "stopped".into(), kept: "half a sentence".into() });
        assert_eq!(done.hide, 4);
    }
    assert_eq!(finish(4, true, Err("stopped".into())).route, Route::Report("stopped".into()));
    assert_eq!(finish(4, true, Ok("hello".into())).route, Route::Insert("hello".into()));
    assert_eq!(finish(4, false, Ok("hello".into())).route, Route::Nothing);
}
