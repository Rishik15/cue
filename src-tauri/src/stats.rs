//! File: stats.rs
//! Purpose: Dictation statistics for the Dictation page: how many words were dictated, how fast, how much typing time that saved, and a
//! per-day count for the activity heatmap. Kept in memory only (no text, no audio) and gone when Cue quits.
//! shortcut: in memory, so history starts empty at every launch; write daily totals to disk if history across launches is wanted.
//! Contents: record — note one finished dictation with its own time zone offset; local_offset / offset_between / days_from_civil — the offset in force now, read from the local clock; summarize — pure: events to the numbers for a range; get_stats — command.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::platform::{self, LocalTime};

const DAY: i64 = 86_400;
/// The heatmap shows a year.
const YEAR_DAYS: usize = 365;
/// A typist's average, used to say how much time dictating saved.
const TYPING_WPM: f64 = 40.0;
/// Enough for years of use at hundreds of dictations a day; the oldest is dropped past it.
const KEEP: usize = 50_000;

/// One finished dictation: when, how many words, how many seconds spoken, and the time zone offset in seconds that held at that moment, so a later clock change cannot move it to another day.
struct Event {
    at: i64,
    words: u32,
    secs: f32,
    tz: i32,
}

static EVENTS: Mutex<VecDeque<Event>> = Mutex::new(VecDeque::new());

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
/// The statistics the Dictation page shows.
pub struct View {
    pub wpm: u32,
    pub words: u64,
    pub speaking_secs: f64,
    pub saved_secs: f64,
    /// Words per day for the last 365 days, oldest first, ending today.
    pub days: Vec<u32>,
}

/// The current Unix time in seconds.
fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// Notes one finished dictation and tells the Dictation page. Inputs: app handle, words dictated, seconds spoken. Does nothing for zero words.
pub fn record(app: &AppHandle, words: u32, secs: f32) {
    if words == 0 {
        return;
    }
    let at = now();
    let mut events = events();
    if events.len() == KEEP {
        events.pop_front();
    }
    events.push_back(Event { at, words, secs, tz: local_offset(at) });
    drop(events);
    let _ = app.emit("stats-changed", ());
}

/// Locks the history; a panic elsewhere must not stop statistics. Returns the guard.
fn events() -> std::sync::MutexGuard<'static, VecDeque<Event>> {
    EVENTS.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Days since 1970-01-01 of a calendar date (proleptic Gregorian). Inputs: year, month 1-12, day 1-31. Returns the day number.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The time zone offset implied by a local clock reading. Inputs: the local reading and the UTC Unix time it was taken at. Returns seconds east of UTC, rounded to 15 minutes (the reading and the time differ by a second or so) and 0 when the result is not a real offset.
fn offset_between(local: &LocalTime, utc: i64) -> i32 {
    let civil = days_from_civil(i64::from(local.year), i64::from(local.month), i64::from(local.day)) * DAY
        + i64::from(local.hour) * 3600
        + i64::from(local.minute) * 60
        + i64::from(local.second);
    let rounded = ((civil - utc) as f64 / 900.0).round() as i64 * 900;
    if rounded.abs() <= 14 * 3600 {
        rounded as i32
    } else {
        0
    }
}

/// The offset from UTC in force at a moment, from Windows' own clock. Input: the Unix time, now. Returns seconds east of UTC.
fn local_offset(utc: i64) -> i32 {
    offset_between(&platform::local_time(), utc)
}

/// The day number of a moment in the user's own time zone (`tz` is its offset from UTC in seconds), so evening dictation lands on the right day.
fn day_of(at: i64, tz: i64) -> i64 {
    (at + tz).div_euclid(DAY)
}

/// How many days back (counting today) a range covers; None is everything.
fn span(range: &str) -> Option<i64> {
    match range {
        "today" => Some(1),
        "week" => Some(7),
        "month" => Some(30),
        _ => None,
    }
}

/// How many days before today an event happened, by the day it was made on. Inputs: the event and today's day number. Returns 0 or more: an event from a time zone further east than the one now (after travelling west) counts as today, in the totals and the heatmap alike.
fn days_back(e: &Event, today: i64) -> i64 {
    (today - day_of(e.at, i64::from(e.tz))).max(0)
}

/// Turns events into the numbers for a range. Inputs: events, the current time, the range name, the time zone offset in seconds that holds now (each event keeps its own). Returns the view.
fn summarize(events: &[Event], now: i64, range: &str, tz: i64) -> View {
    let today = day_of(now, tz);
    let in_range = |e: &&Event| span(range).is_none_or(|n| days_back(e, today) < n);
    let (words, secs) = events.iter().filter(in_range).fold((0u64, 0f64), |(w, s), e| (w + u64::from(e.words), s + f64::from(e.secs)));
    let mut days = vec![0u32; YEAR_DAYS];
    for e in events {
        let back = days_back(e, today);
        if (0..YEAR_DAYS as i64).contains(&back) {
            days[YEAR_DAYS - 1 - back as usize] += e.words;
        }
    }
    View {
        wpm: if secs >= 1.0 { (words as f64 / (secs / 60.0)).round() as u32 } else { 0 },
        words,
        speaking_secs: secs,
        saved_secs: (words as f64 * 60.0 / TYPING_WPM - secs).max(0.0),
        days,
    }
}

#[tauri::command]
/// Command: the statistics for a range. Inputs: range name and the UI's time zone offset in minutes. Returns the view.
pub fn get_stats(range: String, tz_offset_minutes: i32) -> View {
    summarize(events().make_contiguous(), now(), &range, i64::from(tz_offset_minutes) * 60)
}

#[cfg(test)]
mod tests;
