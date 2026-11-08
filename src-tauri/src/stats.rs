//! File: stats.rs
//! Purpose: Dictation statistics for the Dictation page: how many words were dictated, how fast, how much typing time that saved, and a
//! per-day count for the activity heatmap. Kept in memory only (no text, no audio) and gone when Cue quits.
//! shortcut: in memory, so history starts empty at every launch; write daily totals to disk if history across launches is wanted.
//! Contents: record — note one finished dictation; summarize — pure: events to the numbers for a range; get_stats — command.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, Emitter};

const DAY: i64 = 86_400;
/// The heatmap shows a year.
const YEAR_DAYS: usize = 365;
/// A typist's average, used to say how much time dictating saved.
const TYPING_WPM: f64 = 40.0;
/// Enough for years of use at hundreds of dictations a day; the oldest is dropped past it.
const KEEP: usize = 50_000;

/// One finished dictation: when, how many words, how many seconds spoken.
struct Event {
    at: i64,
    words: u32,
    secs: f32,
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
    let mut events = EVENTS.lock().unwrap();
    if events.len() == KEEP {
        events.pop_front();
    }
    events.push_back(Event { at: now(), words, secs });
    drop(events);
    let _ = app.emit("stats-changed", ());
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

/// Turns events into the numbers for a range. Inputs: events, the current time, the range name, the time zone offset in seconds. Returns the view.
fn summarize(events: &[Event], now: i64, range: &str, tz: i64) -> View {
    let today = day_of(now, tz);
    let in_range = |e: &&Event| span(range).is_none_or(|n| today - day_of(e.at, tz) < n);
    let (words, secs) = events.iter().filter(in_range).fold((0u64, 0f64), |(w, s), e| (w + u64::from(e.words), s + f64::from(e.secs)));
    let mut days = vec![0u32; YEAR_DAYS];
    for e in events {
        let back = today - day_of(e.at, tz);
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
    summarize(EVENTS.lock().unwrap().make_contiguous(), now(), &range, i64::from(tz_offset_minutes) * 60)
}

#[cfg(test)]
mod tests;
