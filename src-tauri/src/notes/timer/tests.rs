//! File: notes/timer/tests.rs
//! Purpose: Protect the timer lines of a note: every form the README shows reads as intended, lines that are not timers are left alone, and a pomodoro runs its rounds, rests and finish in order.
//! Contents: tests of `spec::parse`, `spec::control`, `Entry`, and the sweep and liveness the timer thread runs on, with instants moved instead of sleeping.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use super::super::breaks::fake::Fake;
use super::super::breaks::{begin_on, cancel_on, Held};
use super::clock::Clock;
use super::entry::{Entry, Phase};
use super::spec::{control, parse, Control, Kind, Spec};
use super::{live, orphans, plan, sweep, take_pomodoros, wake, Screen, Timers};
use std::sync::Mutex;

/// Ten o'clock in the morning, the time `until` counts from.
const TEN: Clock = Clock { hour: 10, minute: 0, second: 0 };

/// A countdown spec of `secs` seconds with a name. Inputs: seconds and name. Returns the spec.
fn countdown(secs: u64, name: &str) -> Spec {
    Spec { kind: Kind::Countdown, name: name.into(), secs, rest: 0, rounds: 0 }
}

/// A pomodoro spec. Inputs: focus seconds, rest seconds, rounds and name. Returns the spec.
fn pomodoro(secs: u64, rest: u64, rounds: u32, name: &str) -> Spec {
    Spec { kind: Kind::Pomodoro, name: name.into(), secs, rest, rounds }
}

/// A bare timer is a stopwatch; numbers are minutes, with decimals, minutes:seconds, units and spoken-style units all allowed.
#[test]
fn durations_read_in_every_written_form() {
    let stopwatch = Spec { kind: Kind::Stopwatch, name: String::new(), secs: 0, rest: 0, rounds: 0 };
    assert_eq!(parse("timer", TEN), Some(stopwatch));
    for (line, secs) in
        [("timer 5", 300), ("timer 3.5", 210), ("timer 3:30", 210), ("timer 1h30m", 5400), ("timer 90s", 90), ("timer 5 min", 300)]
    {
        assert_eq!(parse(line, TEN), Some(countdown(secs, "")), "{line}");
    }
}

/// A clock time counts down to its next occurrence, tomorrow when it has passed.
#[test]
fn clock_times_count_down_to_the_next_occurrence() {
    assert_eq!(parse("timer until 3:30pm", TEN), Some(countdown(5 * 3600 + 30 * 60, "")));
    assert_eq!(parse("timer 9AM", TEN), Some(countdown(23 * 3600, "")));
    assert_eq!(parse("timer until 10:00", TEN), Some(countdown(24 * 3600, "")), "a time that is now means tomorrow");
    assert_eq!(parse("timer until 25:00", TEN), None);
}

/// Pomodoro defaults to 25 focus and 5 rest, takes its own times and a number of rounds, and a title after the colon names it.
#[test]
fn pomodoro_lines_read_times_rounds_and_names() {
    assert_eq!(parse("timer pomodoro", TEN), Some(pomodoro(1500, 300, 0, "")));
    assert_eq!(parse("timer pomodoro x4", TEN), Some(pomodoro(1500, 300, 4, "")));
    assert_eq!(parse("timer 5 1", TEN), Some(pomodoro(300, 60, 0, "")));
    assert_eq!(parse("timer 5: Laundry", TEN), Some(countdown(300, "Laundry")));
    assert_eq!(parse("timer tea 10", TEN), Some(countdown(600, "tea")));
}

/// Lines that are not timers or ask for nothing sensible are left alone: other text, controls, zero, and more than a day.
#[test]
fn lines_that_are_not_timers_read_as_nothing() {
    for line in ["meeting at 5", "timer p", "timer s", "timer 0", "timer 25 hours"] {
        assert_eq!(parse(line, TEN), None, "{line}");
    }
    assert_eq!(
        (control("timer p"), control("timer r"), control("TIMER s"), control("timer 0")),
        (Some(Control::Pause), Some(Control::Restart), Some(Control::Stop), Some(Control::Stop))
    );
    assert_eq!(control("timer 5"), None);
    assert_eq!(control("timer p now"), None);
}

/// A pomodoro of two rounds: focus, rest, focus, then the finish, with the name on every message and the end only after the last round.
#[test]
fn a_pomodoro_alternates_focus_and_rest_and_ends_after_its_rounds() {
    let mut entry = Entry::new(1, pomodoro(1500, 300, 2, "Study"), "timer pomodoro x2: Study".into());
    assert_eq!(entry.started(), "25/5 Pomodoro started");
    let steps = [
        ("Focus done, time to rest: Study", false, Phase::Rest),
        ("Rest over, back to focus: Study", false, Phase::Focus),
        ("All rounds done: Study", true, Phase::Focus),
    ];
    for (text, done, phase) in steps {
        let said = entry.advance();
        assert_eq!((said.text.as_str(), said.done), (text, done));
        assert!(said.phase == phase);
    }
}

/// A countdown ends with one message, a stopwatch never has a deadline, and pausing removes the deadline until the timer resumes.
#[test]
fn countdowns_end_stopwatches_run_free_and_pause_removes_the_deadline() {
    let mut countdown_entry = Entry::new(1, countdown(60, ""), "timer 1".into());
    assert!(countdown_entry.due().is_some());
    assert_eq!(countdown_entry.toggle_pause(), "Timer paused");
    assert!(countdown_entry.due().is_none());
    assert_eq!(countdown_entry.toggle_pause(), "Timer resumed");
    assert!(countdown_entry.due().is_some());
    let said = countdown_entry.advance();
    assert_eq!((said.text.as_str(), said.done), ("Timer done", true));

    let watch = Entry::new(1, Spec { kind: Kind::Stopwatch, name: String::new(), secs: 0, rest: 0, rounds: 0 }, "timer".into());
    assert!(watch.due().is_none());
    assert_eq!(watch.started(), "Stopwatch started");
}

/// A table of timers with no thread, holding the given (note, entry) pairs.
fn table(entries: Vec<(i64, Entry)>) -> Timers {
    Timers { all: HashMap::from_iter(entries), thread: false }
}

/// A minutes figure too big for a day is refused instead of overflowing.
#[test]
fn oversized_minutes_are_refused_not_overflowed() {
    for line in ["timer 307445734561825861:00", "timer 18446744073709551615:59", "timer 1441:00"] {
        assert_eq!(parse(line, TEN), None, "{line}");
    }
    assert_eq!(parse("timer 1440:00", TEN), Some(countdown(86_400, "")));
    assert_eq!(parse("timer 3:75", TEN), None);
    assert_eq!(parse("timer 3:75 Laundry", TEN), None);
}

/// A word after a number is a name unless it is a unit; am and pm still read as a clock time.
#[test]
fn a_trailing_name_stays_a_name_and_units_still_join() {
    assert_eq!(parse("timer 5 Laundry", TEN), Some(countdown(300, "Laundry")));
    assert_eq!(parse("timer 5 Laundry day", TEN), Some(countdown(300, "Laundry day")));
    assert_eq!(parse("timer 5 Minutes", TEN), Some(countdown(300, "")));
    assert_eq!(parse("timer 10 pm", TEN), Some(countdown(12 * 3600, "")));
}

/// Several phases overdue after a long sleep say only the last, once, with the owner of the timer.
#[test]
fn a_sweep_after_a_long_sleep_says_only_the_last_phase() {
    let entry = Entry::new(7, pomodoro(1, 1, 0, ""), "timer 1s 1s".into());
    let owner = entry.owner;
    let mut timers = table(vec![(7, entry)]);
    let said = sweep(&mut timers, Instant::now() + Duration::from_millis(10_500));
    assert_eq!(said.len(), 1);
    assert_eq!((said[0].owner, said[0].phase, said[0].done), (owner, Phase::Focus, false));
    assert!(timers.all[&7].due().is_some());
}

/// A finished countdown leaves the table, and with the last deadline gone nothing is left for the thread to wait on.
#[test]
fn the_last_deadline_disappearing_leaves_nothing_to_wait_for() {
    let mut timers = table(vec![(1, Entry::new(1, countdown(60, ""), "timer 1".into()))]);
    assert!(sweep(&mut timers, Instant::now()).is_empty());
    let said = sweep(&mut timers, Instant::now() + Duration::from_secs(61));
    assert_eq!((said.len(), said[0].done), (1, true));
    assert!(timers.all.is_empty());
    assert!(timers.all.values().filter_map(Entry::due).min().is_none());
}

/// Only the timer that said a phase may act on it: a stop, a restart or a replacement cancels what it queued; a run-out countdown still counts.
#[test]
fn a_phase_said_by_a_cancelled_timer_is_not_live() {
    let mut timers = table(vec![(1, Entry::new(1, pomodoro(1, 1, 0, ""), "timer 1s 1s".into()))]);
    let said = sweep(&mut timers, Instant::now() + Duration::from_millis(1_500));
    let owner = said[0].owner;
    assert!(live(&timers, owner, false));
    timers.all.get_mut(&1).unwrap().restart();
    assert!(!live(&timers, owner, false), "restarted");
    let fresh = timers.all[&1].owner;
    assert!(live(&timers, fresh, false));
    timers.all.insert(1, Entry::new(1, pomodoro(1, 1, 0, ""), "timer 1s 1s".into()));
    assert!(!live(&timers, fresh, false), "replaced");
    timers.all.remove(&1);
    assert!(!live(&timers, owner, false), "stopped");
    assert!(live(&timers, owner, true), "a countdown that ran out");
}

/// Two notes running pomodoros own different breaks: neither owner equals the other's, and a note's owner changes at each restart.
#[test]
fn two_notes_never_share_a_break_owner() {
    let a = Entry::new(1, pomodoro(5, 30, 0, ""), "timer 5s 30s".into());
    let b = Entry::new(2, pomodoro(10, 60, 0, ""), "timer 10s 60s".into());
    assert_ne!(a.owner, b.owner);
    let mut again = Entry::new(1, pomodoro(5, 30, 0, ""), "timer 5s 30s".into());
    let first = again.owner;
    again.restart();
    assert_ne!(again.owner, first);
}

/// A timer whose thread cannot start leaves no thread flag set; one that can, sets it and keeps it.
#[test]
fn a_failed_thread_start_leaves_the_flag_clear() {
    let mut timers = table(vec![]);
    assert!(!wake(&mut timers, || Err(std::io::Error::other("no thread"))));
    assert!(!timers.thread);
    assert!(wake(&mut timers, || Ok(())));
    assert!(timers.thread);
    assert!(wake(&mut timers, || panic!("already alive")));
}

/// A restart whose thread cannot start closes the break of the run it replaced as well as its own, so no full-screen rest stays up with no timer.
#[test]
fn a_failed_restart_cancels_the_old_break_too() {
    let mut entry = Entry::new(1, pomodoro(1, 1, 0, ""), "timer 1s 1s".into());
    let before = entry.owner;
    entry.restart();
    let now = entry.owner;
    assert_eq!(orphans(Some(entry), Some(before)), vec![now, before]);
    assert_eq!(orphans(None, Some(before)), vec![before]);
}

/// A countdown and a pomodoro of two notes, as the table holds them.
fn two_timers() -> (Timers, super::Owner, super::Owner) {
    let pomodoro = Entry::new(1, pomodoro(5, 30, 0, ""), "timer 5s 30s".into());
    let countdown = Entry::new(2, countdown(60, ""), "timer 1".into());
    let owners = (pomodoro.owner, countdown.owner);
    (table(vec![(1, pomodoro), (2, countdown)]), owners.0, owners.1)
}

/// End Pomodoro removes only the pomodoros and names their owners; the countdown keeps running.
#[test]
fn ending_pomodoros_leaves_countdowns_and_returns_their_owners() {
    let (mut timers, pomodoro, _) = two_timers();
    assert_eq!(take_pomodoros(&mut timers), vec![pomodoro]);
    assert_eq!(timers.all.keys().collect::<Vec<_>>(), vec![&2]);
}

/// A rest announced after End Pomodoro removed its timer opens no break (the scheduler's live check fails), and a break opened just before is closed by the cancel End then makes: either order leaves no window.
#[test]
fn end_pomodoro_leaves_no_break_in_either_order() {
    for open_first in [false, true] {
        let (mut timers, owner, _) = two_timers();
        let (stage, holder): (Fake, Held) = (Fake::default(), Mutex::new(None));
        if open_first {
            begin_on(&stage, &holder, owner, 30, 1, || live(&timers, owner, false));
        }
        take_pomodoros(&mut timers).into_iter().for_each(|o| cancel_on(&stage, &holder, o));
        begin_on(&stage, &holder, owner, 30, 1, || live(&timers, owner, false));
        assert!(!stage.open(), "open_first={open_first}");
    }
}

/// An announcement of a timer that was stopped or replaced is dropped; a live one opens the break for a full-screen rest and ends it at the next focus; other modes leave the break screen alone.
#[test]
fn announcements_follow_the_timer_and_the_alert_mode() {
    let mut entry = Entry::new(1, pomodoro(5, 30, 0, ""), "timer 5s 30s".into());
    let rest = entry.advance();
    let focus = entry.advance();
    let timers = table(vec![(1, entry)]);
    assert_eq!(plan(&timers, &rest, "Full Screen"), Some(Screen::Open));
    assert_eq!(plan(&timers, &rest, "Notification"), Some(Screen::Leave));
    assert_eq!(plan(&timers, &focus, "Full Screen"), Some(Screen::Finish));
    assert_eq!(plan(&table(vec![]), &rest, "Full Screen"), None, "stopped");
    let (replaced, ..) = two_timers();
    assert_eq!(plan(&replaced, &rest, "Full Screen"), None, "another generation owns the note");
}
