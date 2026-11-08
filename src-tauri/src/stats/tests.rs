//! File: stats/tests.rs
//! Purpose: Protect the Dictation page numbers: ranges by the user's own day, words per minute, time saved and the year of daily counts.
//! Contents: tests of `summarize`.

use super::{days_from_civil, offset_between, summarize, Event, DAY, YEAR_DAYS};
use crate::platform::LocalTime;

/// An hour of dictation spread over three days: the range names pick the right days in the user's time zone, and the heatmap puts each day's words in its slot.
#[test]
fn ranges_count_days_in_the_users_time_zone_and_fill_the_heatmap() {
    let tz = -5 * 3600;
    let now = 10 * DAY + 20 * 3600;
    let events = [
        Event { at: now - 2 * DAY, words: 100, secs: 60.0, tz: tz as i32 },
        Event { at: now - DAY, words: 200, secs: 120.0, tz: tz as i32 },
        Event { at: now, words: 40, secs: 30.0, tz: tz as i32 },
    ];
    let today = summarize(&events, now, "today", tz);
    assert_eq!((today.words, today.days[YEAR_DAYS - 1]), (40, 40));
    assert_eq!(today.wpm, 80);
    let week = summarize(&events, now, "week", tz);
    assert_eq!(week.words, 340);
    assert_eq!(&week.days[YEAR_DAYS - 3..], &[100, 200, 40]);
    let all = summarize(&events, now, "all", tz);
    assert_eq!(all.words, 340);
    assert!((all.speaking_secs - 210.0).abs() < 1e-6);
}

/// Late evening in a western time zone still belongs to that local day, not tomorrow's UTC day.
#[test]
fn evening_dictation_lands_on_the_local_day() {
    let tz = -8 * 3600;
    let utc_next_morning = 5 * DAY + 3 * 3600;
    let events = [Event { at: utc_next_morning, words: 10, secs: 5.0, tz: tz as i32 }];
    let view = summarize(&events, utc_next_morning + 3600, "today", tz);
    assert_eq!(view.days[YEAR_DAYS - 1], 10);
    let next_local_day = summarize(&events, utc_next_morning + 24 * 3600, "today", tz);
    assert_eq!(next_local_day.words, 0);
}

/// Nothing dictated gives zeros, not a division by zero; typing time saved never goes negative.
#[test]
fn an_empty_history_and_slow_speech_give_sane_numbers() {
    let empty = summarize(&[], 1_000_000, "all", 0);
    assert_eq!((empty.wpm, empty.words), (0, 0));
    assert_eq!(empty.saved_secs, 0.0);
    let slow = summarize(&[Event { at: 1_000_000, words: 2, secs: 600.0, tz: 0 }], 1_000_000, "all", 0);
    assert_eq!(slow.saved_secs, 0.0);
}

/// A dictation just after midnight in New York stays on its own day after the clocks fall back, because the event keeps the offset that held when it was made.
#[test]
fn a_clock_change_does_not_move_past_dictation() {
    let nov_7_0430_utc = days_from_civil(2026, 11, 7) * DAY + 4 * 3600 + 30 * 60;
    let nov_8_noon_utc = days_from_civil(2026, 11, 8) * DAY + 17 * 3600;
    let events = [Event { at: nov_7_0430_utc, words: 10, secs: 5.0, tz: -4 * 3600 }];
    let view = summarize(&events, nov_8_noon_utc, "all", -5 * 3600);
    assert_eq!(view.days[YEAR_DAYS - 2], 10);
}

/// Calendar dates convert to the right day numbers, including a leap day and a date before 1970.
#[test]
fn civil_dates_become_day_numbers() {
    assert_eq!(days_from_civil(1970, 1, 1), 0);
    assert_eq!(days_from_civil(2024, 3, 1), 19_783);
    assert_eq!(days_from_civil(1969, 12, 31), -1);
}

/// A local clock reading a second after the UTC time rounds to the zone's whole offset; an impossible one gives zero.
#[test]
fn the_offset_is_read_from_the_local_clock() {
    let utc = days_from_civil(2026, 11, 8) * DAY + 17 * 3600;
    let local = LocalTime { year: 2026, month: 11, day: 8, hour: 12, minute: 0, second: 1, weekday: 0 };
    assert_eq!(offset_between(&local, utc), -5 * 3600);
    let india = LocalTime { year: 2026, month: 11, day: 8, hour: 22, minute: 30, second: 0, weekday: 0 };
    assert_eq!(offset_between(&india, utc), 5 * 3600 + 1800);
    let nonsense = LocalTime { year: 1970, month: 1, day: 1, ..Default::default() };
    assert_eq!(offset_between(&nonsense, utc), 0);
}

/// After travelling west, a dictation made in a zone further east counts as today in the totals and the heatmap alike, not as a future day.
#[test]
fn an_event_from_a_later_local_day_counts_as_today() {
    let now = 10 * DAY + 3 * 3600;
    let events = [Event { at: now, words: 10, secs: 5.0, tz: 9 * 3600 }];
    let view = summarize(&events, now, "today", -8 * 3600);
    assert_eq!((view.words, view.days[YEAR_DAYS - 1]), (10, 10));
}
