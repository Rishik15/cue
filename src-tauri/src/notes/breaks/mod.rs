//! File: notes/breaks/mod.rs
//! Purpose: The full-screen break alert of a pomodoro: a window built when a rest starts and destroyed when it ends, so nothing is resident while idle. It also keeps the no-peek streak.
//! Contents: Owner — the timer (note and generation) a break belongs to; begin — show the alert for a rest of `rest_secs` after `rounds` focus periods; end — that timer's rest is over: count a streak day and close it; cancel — that timer was stopped, restarted or replaced: close it, no streak; break_act — the page's one command (it painted; Sneak a peek; End Pomodoro).
//! The streak and best are written by Rust alone (`notes_break_streak`, `notes_break_best`), so they are not in `settings::validate`.

mod window;

use std::sync::{Mutex, MutexGuard};

use tauri::AppHandle;

use crate::{settings, status};

const STREAK: &str = "notes_break_streak";
const BEST: &str = "notes_break_best";

/// The timer a break belongs to: only that timer ends or cancels it, so one note's timer never closes another's break.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Owner {
    pub note: i64,
    pub generation: u64,
}

/// Who owns the break on screen (held while the window is built or closed, so a begin and an end never interleave).
static HELD: Mutex<Option<Owner>> = Mutex::new(None);

/// The owner of the break on screen, locked.
fn held() -> MutexGuard<'static, Option<Owner>> {
    HELD.lock().unwrap_or_else(|e| e.into_inner())
}

/// Closes the break on screen whoever owns it (the page's own buttons).
fn dismiss(app: &AppHandle) {
    let mut held = held();
    window::close(app);
    *held = None;
}

/// Shows the break alert for a rest of `owner`'s timer. Inputs: app handle, owner, rest seconds, focus rounds done and `live`, asked once the break is locked in: a timer cancelled meanwhile shows nothing. A failure is shown to the user.
pub fn begin(app: &AppHandle, owner: Owner, rest_secs: u64, rounds: u32, live: impl FnOnce() -> bool) {
    let mut held = held();
    if !live() {
        return;
    }
    window::close(app);
    let data = serde_json::json!({
        "rest_secs": rest_secs,
        "rounds": rounds,
        "streak": settings::get_u64(STREAK, 0),
        "best": settings::get_u64(BEST, 0),
        "theme": settings::get_theme(),
    });
    match window::build(app, format!("window.__BREAK__ = {data};")) {
        Ok(()) => *held = Some(owner),
        Err(e) => {
            *held = None;
            status::report(app, &format!("Could not show the break screen: {e}"));
        }
    }
}

/// `owner`'s rest ended with the window still up, so it was not peeked at: the streak grows. Another timer's break is left alone.
pub fn end(app: &AppHandle, owner: Owner) {
    let mut held = held();
    if *held == Some(owner) && window::exists(app) {
        record(app, settings::get_u64(STREAK, 0) + 1);
    }
    close_if_owned(app, &mut held, owner);
}

/// `owner`'s timer was stopped, restarted or replaced during its rest: the break closes and earns nothing. Another timer's break is left alone.
pub fn cancel(app: &AppHandle, owner: Owner) {
    close_if_owned(app, &mut held(), owner);
}

/// Closes the break when `owner` holds it.
fn close_if_owned(app: &AppHandle, held: &mut Option<Owner>, owner: Owner) {
    if *held == Some(owner) {
        window::close(app);
        *held = None;
    }
}

/// Saves the no-peek streak and the best one in one save. Inputs: app handle and the new streak. Settings from a newer Cue are left alone; a failed save is shown to the user.
fn record(app: &AppHandle, streak: u64) {
    if settings::is_newer() {
        return;
    }
    let best = settings::get_u64(BEST, 0).max(streak);
    if let Err(e) = settings::update(&[(STREAK, Some(streak.into())), (BEST, Some(best.into()))]) {
        status::report(app, &format!("Could not save the no-peek streak: {e}"));
    }
}

/// What the break page asks for.
#[derive(serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Act {
    /// The page has painted: show the window.
    Ready,
    /// Sneak a peek: the streak starts over; the pomodoro goes on.
    Peek,
    /// End Pomodoro: close the alert and stop the pomodoro with it.
    End,
}

/// The page's one command. Inputs: the app handle and the act.
#[tauri::command]
pub fn break_act(app: AppHandle, act: Act) {
    match act {
        Act::Ready => window::show(&app),
        Act::Peek => {
            record(&app, 0);
            dismiss(&app);
        }
        Act::End => {
            dismiss(&app);
            super::timer::stop_all_pomodoros(&app);
        }
    }
}
