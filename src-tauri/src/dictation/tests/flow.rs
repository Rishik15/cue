//! File: dictation/tests/flow.rs
//! Purpose: Protect who owns a dictation and how it ends: the reducer in `flow` fed the event sequences that raced in the old code (a failure before the id is published, a cap against a stop, Escape against a stop, a late callback or regrip timer after a new start), the key state (regrip, bounce, Hold and Toggle stops), the recording's snapshot and a failure after partial text.
//! Contents: tests of `State::apply` and `finish`.

use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::audio::{End, Loss};
use crate::dictation::flow::{finish, Effect, Event, Snapshot, State, Timer};
use crate::dictation::mode::Mode;
use crate::dictation::routing::Route;
use crate::platform::Target;
use crate::speech::Failure;

/// A field with a window number, to tell two recordings apart.
fn field(window: isize) -> Target {
    Target { window, ..Target::default() }
}

/// A snapshot of a recording's settings.
fn snapshot() -> Snapshot {
    Snapshot { show_overlay: true, cleaner: Arc::default() }
}

/// A state with recording `id` running in window `window`.
fn recording(id: u64, window: isize) -> State {
    let mut state = State::new();
    assert_eq!(state.apply(Event::Start(field(window))), [Effect::BeginSpeech]);
    assert_eq!(state.apply(Event::Published(id, snapshot())), [Effect::OpenMic(id)]);
    state
}

/// A key press or release at `at`.
fn key(state: &mut State, down: bool, at: Instant, mode: Mode) -> Vec<Effect> {
    state.apply(Event::Key { down, at, mode })
}

/// The one timer among `effects`, last of them.
fn timer(effects: &[Effect]) -> Timer {
    match effects.last() {
        Some(Effect::Timer(timer)) => *timer,
        other => panic!("no timer in {other:?}"),
    }
}

/// The effect that ends recording `id` in window `window` with the snapshot `state` holds for it.
fn stop_mic(state: &State, id: u64, window: isize) -> Effect {
    Effect::StopMic { id, target: field(window), snapshot: state.caption_snapshot(id) }
}

/// A confirmed stop of the running recording `id`: a Toggle press and the end of its wait. Returns the effects of the end of the wait.
fn stop(state: &mut State, id: u64) -> Vec<Effect> {
    let effects = key(state, true, Instant::now(), Mode::Toggle);
    assert_eq!(effects.len(), 1);
    assert_eq!(timer(&effects).id, id);
    state.apply(Event::Regrip(timer(&effects)))
}

/// A press during a recording or its ending starts nothing; once the stop is issued the next press starts.
#[test]
fn a_press_during_a_recording_or_its_ending_is_dropped() {
    let mut state = recording(1, 7);
    assert!(state.apply(Event::Start(field(8))).is_empty());
    let expected = stop_mic(&state, 1, 7);
    assert_eq!(stop(&mut state, 1), [expected]);
    assert!(state.apply(Event::Start(field(8))).is_empty(), "still ending");
    state.apply(Event::StopIssued(Instant::now()));
    assert_eq!(state.apply(Event::Start(field(8))), [Effect::BeginSpeech]);
}

/// In Hold mode a release that stays up through the regrip window ends the recording.
#[test]
fn a_hold_release_without_a_new_press_confirms_the_stop() {
    let mut state = recording(1, 7);
    let t0 = Instant::now();
    let effects = key(&mut state, false, t0 + Duration::from_secs(2), Mode::Hold);
    assert_eq!(timer(&effects), Timer { ticket: 1, id: 1, from_release: true });
    let expected = stop_mic(&state, 1, 7);
    assert_eq!(state.apply(Event::Regrip(timer(&effects))), [expected]);
}

/// A Toggle press that stops needs no key to stay up: its wait confirms.
#[test]
fn a_toggle_stop_confirms() {
    let mut state = recording(1, 7);
    let expected = stop_mic(&state, 1, 7);
    assert_eq!(stop(&mut state, 1), [expected]);
}

/// A press inside the regrip window is the key coming back: it returns nothing, and the wait that follows leaves the recording running.
#[test]
fn a_press_inside_the_regrip_window_continues_the_recording() {
    let mut state = recording(1, 7);
    let t0 = Instant::now();
    let effects = key(&mut state, false, t0, Mode::Hold);
    assert!(key(&mut state, true, t0 + Duration::from_millis(50), Mode::Hold).is_empty());
    assert!(state.apply(Event::Regrip(timer(&effects))).is_empty());
    assert!(state.is_recording());
    assert!(
        key(&mut state, true, t0 + Duration::from_secs(1), Mode::Hold).is_empty(),
        "the window is over: a press while recording starts nothing"
    );
}

/// A press right after a stop was issued is key bounce and starts nothing; a later one starts.
#[test]
fn a_press_within_the_bounce_of_a_stop_starts_nothing() {
    let mut state = recording(1, 7);
    stop(&mut state, 1);
    let t0 = Instant::now();
    state.apply(Event::StopIssued(t0));
    assert!(key(&mut state, true, t0 + Duration::from_millis(40), Mode::Hold).is_empty());
    assert_eq!(key(&mut state, true, t0 + Duration::from_millis(100), Mode::Hold), [Effect::StartRequested]);
}

/// Recording A is stopped and recording B started; A's wait ends after that and changes nothing.
#[test]
fn stop_a_then_start_b_ignores_a_late_timer() {
    let mut state = recording(1, 7);
    let late = timer(&key(&mut state, false, Instant::now(), Mode::Hold));
    assert_eq!(state.apply(Event::Abandon(Some(1))), [Effect::Discard]);
    let t0 = Instant::now();
    state.apply(Event::StopIssued(t0));
    assert_eq!(key(&mut state, true, t0 + Duration::from_millis(100), Mode::Hold), [Effect::StartRequested]);
    state.apply(Event::Start(field(8)));
    assert_eq!(state.apply(Event::Published(2, snapshot())), [Effect::OpenMic(2)]);
    assert!(state.apply(Event::Regrip(late)).is_empty());
    assert!(state.is_recording());
}

/// The snapshot belongs to its recording: a newer publication replaces it, Finished and Escape clear it, and an older id's Finished cannot clear a newer one.
#[test]
fn the_snapshot_lives_and_dies_with_its_recording() {
    let mut state = recording(1, 7);
    assert!(state.caption_snapshot(1).is_some() && state.caption_snapshot(2).is_none());
    stop(&mut state, 1);
    state.apply(Event::StopIssued(Instant::now()));
    state.apply(Event::Start(field(8)));
    state.apply(Event::Published(2, snapshot()));
    assert!(state.caption_snapshot(1).is_none(), "replaced by the newer publication");
    state.apply(Event::Finished(1));
    assert!(state.caption_snapshot(2).is_some(), "an older Finished leaves the newer snapshot");
    state.apply(Event::Finished(2));
    assert!(state.caption_snapshot(2).is_none());
    let mut state = recording(3, 7);
    state.apply(Event::Abandon(None));
    assert!(state.caption_snapshot(3).is_none());
}

/// The engine fails before the recording has published its id: nothing happens yet, and publishing the same id opens the microphone and asks for its stop, forced past the regrip check.
#[test]
fn a_failure_before_the_id_is_published_ends_the_recording_when_it_is() {
    let mut state = State::new();
    state.apply(Event::Start(field(7)));
    assert!(state.apply(Event::Failed(5)).is_empty());
    let effects = state.apply(Event::Published(5, snapshot()));
    assert_eq!(effects[0], Effect::OpenMic(5));
    let expected = stop_mic(&state, 5, 7);
    assert_eq!(state.apply(Event::Regrip(timer(&effects))), [expected]);
}

/// A failure of another recording's engine, before or after publication, ends nothing.
#[test]
fn a_failure_of_another_recording_is_ignored() {
    let mut state = State::new();
    state.apply(Event::Start(field(7)));
    state.apply(Event::Failed(4));
    assert_eq!(state.apply(Event::Published(5, snapshot())), [Effect::OpenMic(5)]);
    assert!(state.apply(Event::Failed(4)).is_empty());
    assert_eq!(timer(&state.apply(Event::Failed(5))).id, 5);
}

/// The cap and a stop on the same recording: the cap reports once and asks for a stop; of the two waits that follow, one takes the field and the other finds nothing.
#[test]
fn a_cap_and_a_stop_end_the_recording_once() {
    let mut state = recording(1, 7);
    let effects = state.apply(Event::Ended(1, End::Cap));
    assert_eq!(effects[0], Effect::LimitReached);
    let expected = stop_mic(&state, 1, 7);
    assert_eq!(state.apply(Event::Regrip(timer(&effects))), [expected]);
    assert!(state.apply(Event::Regrip(timer(&effects))).is_empty());
    assert!(state.apply(Event::Ended(1, End::Cap)).is_empty(), "no second message once it is ending");
}

/// The recording ends by itself (cap, deadline, broken stream, tick failure), then the user releases inside the regrip window: the release supersedes the end's wait, and that wait still stops the microphone. Only a limit gives the notice.
#[test]
fn an_end_of_the_recording_is_a_final_stop_a_regrip_cannot_cancel() {
    for (end, notice) in [(End::Cap, true), (End::Deadline, true), (End::Broken, false), (End::TickFailed, false)] {
        let mut state = recording(1, 7);
        let effects = state.apply(Event::Ended(1, end));
        assert_eq!(effects.contains(&Effect::LimitReached), notice, "{end:?}");
        let release = key(&mut state, false, Instant::now(), Mode::Hold);
        let expected = stop_mic(&state, 1, 7);
        assert_eq!(state.apply(Event::Regrip(timer(&effects))), [expected]);
        assert!(state.apply(Event::Regrip(timer(&release))).is_empty(), "the second wait finds nothing");
    }
}

/// An end of another recording changes nothing.
#[test]
fn an_end_of_another_recording_is_ignored() {
    let mut state = recording(1, 7);
    assert!(state.apply(Event::Ended(2, End::Broken)).is_empty());
    let superseded = Timer { ticket: 99, id: 1, from_release: false };
    assert!(state.apply(Event::Regrip(superseded)).is_empty(), "no latch was set");
}

/// A lossy recording with words routes to the rescue through `finish`; a cap (no loss) still inserts.
#[test]
fn a_lossy_end_with_words_routes_to_rescue() {
    let done = finish(4, true, Some(Loss::Broken), Ok("half".into()));
    assert_eq!(done.route, Route::Rescue { message: "The microphone stopped working.".into(), kept: "half".into() });
    assert_eq!(finish(4, true, None, Ok("whole".into())).route, Route::Insert("whole".into()));
}

/// Escape and a stop on the same recording: whichever comes first owns the ending, the other does nothing.
#[test]
fn escape_and_a_stop_end_the_recording_once() {
    let mut state = recording(1, 7);
    let effects = key(&mut state, true, Instant::now(), Mode::Toggle);
    assert_eq!(state.apply(Event::Abandon(None)), [Effect::Discard]);
    assert!(state.apply(Event::Regrip(timer(&effects))).is_empty());
    let mut state = recording(2, 7);
    assert_eq!(stop(&mut state, 2).len(), 1);
    assert!(state.apply(Event::Abandon(None)).is_empty());
}

/// The id is published after Escape took the recording: the microphone is never opened for it, whether the ending is still running or already over.
#[test]
fn a_published_id_after_escape_opens_no_microphone() {
    let mut state = State::new();
    state.apply(Event::Start(field(7)));
    assert_eq!(state.apply(Event::Abandon(None)), [Effect::Discard]);
    assert!(state.apply(Event::Published(1, snapshot())).is_empty(), "while stopping");
    state.apply(Event::StopIssued(Instant::now()));
    assert!(state.apply(Event::Published(1, snapshot())).is_empty(), "once idle");
}

/// Callbacks of recording 1 that arrive after recording 2 started change nothing, also while 2 has not published its id yet.
#[test]
fn a_late_callback_of_an_earlier_recording_leaves_the_next_one_alone() {
    let mut state = recording(1, 7);
    let first = key(&mut state, true, Instant::now(), Mode::Toggle);
    state.apply(Event::Regrip(timer(&first)));
    state.apply(Event::StopIssued(Instant::now()));
    state.apply(Event::Start(field(8)));
    let late = |state: &mut State| {
        [Event::Ended(1, End::Cap), Event::Failed(1), Event::Abandon(Some(1)), Event::Regrip(timer(&first))]
            .into_iter()
            .all(|e| state.apply(e).is_empty())
    };
    assert!(late(&mut state), "before the new id");
    assert_eq!(state.apply(Event::Published(2, snapshot())), [Effect::OpenMic(2)]);
    assert!(late(&mut state), "after the new id");
    assert!(state.is_recording());
    let expected = stop_mic(&state, 2, 8);
    assert_eq!(stop(&mut state, 2), [expected]);
}

/// A caption belongs to the recording that published the id, also while it is being ended, and to no other.
#[test]
fn a_caption_shows_only_for_its_own_recording() {
    let mut state = recording(1, 7);
    assert!(state.is_current(1) && !state.is_current(2));
    stop(&mut state, 1);
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
        let done = finish(4, keep, None, Err(failure));
        assert_eq!(done.route, Route::Rescue { message: "stopped".into(), kept: "half a sentence".into() });
        assert_eq!(done.hide, 4);
    }
    assert_eq!(finish(4, true, None, Err("stopped".into())).route, Route::Report("stopped".into()));
    assert_eq!(finish(4, true, None, Ok("hello".into())).route, Route::Insert("hello".into()));
    assert_eq!(finish(4, false, None, Ok("hello".into())).route, Route::Nothing);
}
