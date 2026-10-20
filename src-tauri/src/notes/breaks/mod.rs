//! Purpose: The full-screen break alert of a pomodoro: a window built when a rest starts and destroyed when it ends, so nothing is resident while idle. It also keeps the no-peek streak.
//! Contents: begin — show the alert for a rest of `rest_secs` after `rounds` focus periods; end — the rest is over: count a streak day and close it; break_act — the page's one command (it painted; Sneak a peek; End Pomodoro).
//! The streak and best are written by Rust alone (`notes_break_streak`, `notes_break_best`), so they are not in `settings::validate`.

mod window;

use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

use crate::{settings, status};

const STREAK: &str = "notes_break_streak";
const BEST: &str = "notes_break_best";

pub fn begin(app: &AppHandle, rest_secs: u64, rounds: u32) {
    window::close(app);
    let data = serde_json::json!({
        "rest_secs": rest_secs,
        "rounds": rounds,
        "streak": settings::get_u64(app, STREAK, 0),
        "best": settings::get_u64(app, BEST, 0),
        "theme": settings::get_theme(app.clone()),
    });
    if let Err(e) = window::build(app, format!("window.__BREAK__ = {data};")) {
        status::report(app, &format!("Could not show the break screen: {e}"));
    }
}

/// A rest that ends with the window still up was not peeked at: the streak grows.
pub fn end(app: &AppHandle) {
    if window::exists(app) {
        record(app, settings::get_u64(app, STREAK, 0) + 1);
        window::close(app);
    }
}

fn record(app: &AppHandle, streak: u64) {
    let Ok(store) = app.store(settings::STORE) else { return };
    store.set(STREAK, streak);
    store.set(BEST, settings::get_u64(app, BEST, 0).max(streak));
    if let Err(e) = store.save() {
        status::report(app, &format!("Could not save the no-peek streak: {e}"));
    }
}

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

#[tauri::command]
pub fn break_act(app: AppHandle, act: Act) {
    match act {
        Act::Ready => window::show(&app),
        Act::Peek => {
            record(&app, 0);
            window::close(&app);
        }
        Act::End => {
            window::close(&app);
            super::timer::stop_all_pomodoros(&app);
        }
    }
}
