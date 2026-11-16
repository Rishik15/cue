//! File: platform/tests/watcher_state.rs
//! Purpose: Protect the watcher lifecycle: a start that timed out keeps its slot until its thread has ended, a failed start can be tried again, a stop during a start cancels that generation, and an old generation never touches a newer one.
//! Contents: scenario tests of `Machine`.

use crate::platform::watcher_state::{Effect, Event, Machine, State};

/// A machine that has been started once; returns it and the generation.
fn starting() -> (Machine, u64) {
    let mut m = Machine::new();
    let Effect::Spawn(g) = m.apply(Event::Start)[0] else { panic!("no spawn") };
    (m, g)
}

/// A start that timed out keeps ownership: a new start is refused until the old thread has cleaned up, so the two never share the process-wide registration; its late ready is told to abandon.
#[test]
fn a_timed_out_start_owns_the_slot_until_its_thread_ends() {
    let (mut m, g) = starting();
    assert!(m.apply(Event::StartTimedOut(g)).is_empty());
    assert_eq!(m.state(), State::Stopping(g));
    assert_eq!(m.apply(Event::Start), vec![Effect::Refuse]);
    assert_eq!(m.apply(Event::Ready(g, 7)), vec![Effect::Abandon(g)]);
    assert_eq!(m.state(), State::Stopping(g));
    assert!(m.apply(Event::Ended(g)).is_empty());
    assert_eq!(m.state(), State::Stopped);
    assert_eq!(m.apply(Event::Start), vec![Effect::Spawn(g + 1)]);
}

/// A timed out start whose thread then fails frees the slot for a fresh start.
#[test]
fn a_timed_out_start_that_fails_frees_the_slot() {
    let (mut m, g) = starting();
    m.apply(Event::StartTimedOut(g));
    m.apply(Event::StartFailed(g));
    assert_eq!(m.state(), State::Stopped);
    assert_eq!(m.apply(Event::Start), vec![Effect::Spawn(g + 1)]);
}

/// A failed start returns to Stopped and the next start is a fresh generation.
#[test]
fn failed_start_returns_to_stopped() {
    let (mut m, g) = starting();
    m.apply(Event::StartFailed(g));
    assert_eq!(m.state(), State::Stopped);
    assert_eq!(m.apply(Event::Start), vec![Effect::Spawn(g + 1)]);
    assert_eq!(m.apply(Event::Ready(g + 1, 8)), vec![Effect::Proceed(g + 1)]);
    assert_eq!(m.state(), State::Running(g + 1));
}

/// A stop while starting cancels that generation: it never reaches Running, its thread is told to abandon, and its end returns to Stopped.
#[test]
fn stop_during_start_cancels_that_generation() {
    let (mut m, g) = starting();
    assert!(m.apply(Event::Stop).is_empty());
    assert_eq!(m.state(), State::Stopping(g));
    assert_eq!(m.apply(Event::Ready(g, 7)), vec![Effect::Abandon(g)]);
    assert_eq!(m.state(), State::Stopping(g));
    m.apply(Event::Ended(g));
    assert_eq!(m.state(), State::Stopped);
}

/// A thread of an older start that wakes or ends late cannot revive or clear a newer watcher.
#[test]
fn an_old_generation_never_touches_a_newer_one() {
    let (mut m, old) = starting();
    m.apply(Event::Stop);
    m.apply(Event::Ended(old));
    let Effect::Spawn(new) = m.apply(Event::Start)[0] else { panic!("no spawn") };
    assert_eq!(m.apply(Event::Ready(old, 7)), vec![Effect::Abandon(old)]);
    assert_eq!(m.state(), State::Starting(new));
    m.apply(Event::Ended(old));
    assert_eq!(m.state(), State::Starting(new));
    assert_eq!(m.apply(Event::Ready(new, 8)), vec![Effect::Proceed(new)]);
}

/// A start while running does nothing, a stop asks the loop to quit, and a start during that is refused.
#[test]
fn running_stops_and_refuses_a_start_while_closing() {
    let (mut m, g) = starting();
    m.apply(Event::Ready(g, 7));
    assert!(m.apply(Event::Start).is_empty());
    assert_eq!(m.apply(Event::Stop), vec![Effect::Quit(g, 7)]);
    assert_eq!(m.apply(Event::Start), vec![Effect::Refuse]);
    m.apply(Event::Ended(g));
    assert_eq!(m.state(), State::Stopped);
}

/// A timeout that races a ready counts as started and keeps the accepted thread id, so the stop still reaches that thread; a quit that could not be posted leaves the watcher running.
#[test]
fn a_raced_ready_keeps_its_thread_and_a_failed_quit_keeps_running() {
    let (mut m, g) = starting();
    m.apply(Event::Ready(g, 7));
    assert_eq!(m.apply(Event::StartTimedOut(g)), vec![Effect::Proceed(g)]);
    assert_eq!(m.apply(Event::Stop), vec![Effect::Quit(g, 7)]);
    m.apply(Event::QuitFailed(g));
    assert_eq!(m.state(), State::Running(g));
    assert_eq!(m.apply(Event::Stop), vec![Effect::Quit(g, 7)]);
}

/// A thread that dies before answering reports Ended from its drop guard; the start's own disconnect timeout then changes nothing, the slot is free, and a repeated Ended cannot clear the next generation.
#[test]
fn a_thread_that_dies_before_answering_frees_the_slot() {
    let (mut m, g) = starting();
    m.apply(Event::Ended(g));
    assert_eq!(m.state(), State::Stopped);
    assert!(m.apply(Event::StartTimedOut(g)).is_empty());
    assert_eq!(m.state(), State::Stopped);
    assert_eq!(m.apply(Event::Start), vec![Effect::Spawn(g + 1)]);
    m.apply(Event::Ended(g));
    assert_eq!(m.state(), State::Starting(g + 1));
}
