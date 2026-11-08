//! File: notes/timer/tests.rs
//! Purpose: Protect the timer lines of a note: every form the README shows reads as intended, lines that are not timers are left alone, and a pomodoro runs its rounds, rests and finish in order.
//! Contents: tests of `spec::parse`, `spec::control` and `Entry`.

use super::clock::Clock;
use super::entry::{Entry, Phase};
use super::spec::{control, parse, Control, Kind, Spec};

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
    let mut entry = Entry::new(pomodoro(1500, 300, 2, "Study"), "timer pomodoro x2: Study".into());
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
    let mut countdown_entry = Entry::new(countdown(60, ""), "timer 1".into());
    assert!(countdown_entry.due().is_some());
    assert_eq!(countdown_entry.toggle_pause(), "Timer paused");
    assert!(countdown_entry.due().is_none());
    assert_eq!(countdown_entry.toggle_pause(), "Timer resumed");
    assert!(countdown_entry.due().is_some());
    let said = countdown_entry.advance();
    assert_eq!((said.text.as_str(), said.done), ("Timer done", true));

    let watch = Entry::new(Spec { kind: Kind::Stopwatch, name: String::new(), secs: 0, rest: 0, rounds: 0 }, "timer".into());
    assert!(watch.due().is_none());
    assert_eq!(watch.started(), "Stopwatch started");
}
