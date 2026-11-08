//! File: stats/tests.rs
//! Purpose: Protect the Dictation page numbers: ranges by the user's own day, words per minute, time saved and the year of daily counts.
//! Contents: tests of `summarize`.

use super::{summarize, Event, DAY, YEAR_DAYS};

/// An hour of dictation spread over three days: the range names pick the right days in the user's time zone, and the heatmap puts each day's words in its slot.
#[test]
fn ranges_count_days_in_the_users_time_zone_and_fill_the_heatmap() {
    let tz = -5 * 3600;
    let now = 10 * DAY + 20 * 3600;
    let events = [
        Event { at: now - 2 * DAY, words: 100, secs: 60.0 },
        Event { at: now - DAY, words: 200, secs: 120.0 },
        Event { at: now, words: 40, secs: 30.0 },
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
    let events = [Event { at: utc_next_morning, words: 10, secs: 5.0 }];
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
    let slow = summarize(&[Event { at: 1_000_000, words: 2, secs: 600.0 }], 1_000_000, "all", 0);
    assert_eq!(slow.saved_secs, 0.0);
}
