//! Purpose: Timers for notes. A line `timer ...` (see `spec`) in a note starts a stopwatch, a countdown or a pomodoro that goes on while the window is closed: Rust keeps the deadline, one thread sleeps until the next one is due, says so (a notice, a beep or a full-screen break, per the alert settings) and ends when nothing is left to wait for. One timer per note.
//! Contents: set — start, keep or report the timer of a note; control — pause, restart or stop it; stop_all_pomodoros — the break window's end button; the thread (`run`) is started when a timer with a deadline starts or resumes and ends with the last; announce — the alert of a phase change. Timers live in memory only.

mod clock;
mod entry;
mod spec;

use std::collections::HashMap;
use std::sync::{Condvar, LazyLock, Mutex, MutexGuard};
use std::time::Instant;

use tauri::{AppHandle, Emitter};

use super::breaks;
use crate::platform::{self, overlay};
use crate::prefs::Prefs;
use crate::{guard, settings};
use clock::Clock;
pub use entry::View;
use entry::{Entry, Phase, Said};
pub use spec::Control;
use spec::Kind;

struct Timers {
    all: HashMap<i64, Entry>,
    /// The thread is alive (decided under this lock, so a timer set as it ends is never left without one).
    thread: bool,
}

static TIMERS: LazyLock<Mutex<Timers>> = LazyLock::new(|| Mutex::new(Timers { all: HashMap::new(), thread: false }));
static WAKE: Condvar = Condvar::new();

fn lock() -> MutexGuard<'static, Timers> {
    TIMERS.lock().unwrap_or_else(|e| e.into_inner())
}

/// Wakes the thread, starting it when it is not alive.
fn wake(app: &AppHandle, timers: &mut Timers) {
    if !timers.thread {
        timers.thread = true;
        let handle = app.clone();
        if guard::spawn("note-timers", 128 * 1024, move || run(&handle)).is_err() {
            timers.thread = false;
        }
    }
    WAKE.notify_all();
}

/// A timer that is gone (replaced, stopped, finished): a break on screen goes with it.
fn ended(app: &AppHandle, old: &Entry) {
    if old.phase == Phase::Rest {
        breaks::end(app);
    }
}

/// The timer of note `id`, for `line` (a line of the note the cursor just left; None only reports). A line that is a control word acts on the timer, the same line as the running timer changes nothing, another timer line replaces it.
pub fn set(app: &AppHandle, id: i64, line: Option<&str>) -> Option<View> {
    let Some(line) = line else { return lock().all.get(&id).map(Entry::view) };
    if let Some(ctl) = spec::control(line) {
        return control(app, id, ctl);
    }
    let clock = platform::local_time();
    let parsed = spec::parse(line, Clock { hour: clock.hour, minute: clock.minute, second: clock.second });
    let mut timers = lock();
    let text = line.trim().to_string();
    let Some(spec) = parsed.filter(|_| timers.all.get(&id).is_none_or(|e| e.line != text)) else {
        return timers.all.get(&id).map(Entry::view);
    };
    let entry = Entry::new(spec, text);
    let view = entry.view().with_toast(entry.started());
    let old = timers.all.insert(id, entry);
    wake(app, &mut timers);
    drop(timers);
    old.iter().for_each(|o| ended(app, o));
    Some(view)
}

/// Pause (toggles), restart or stop the timer of note `id`. A stopped timer is gone: None.
pub fn control(app: &AppHandle, id: i64, ctl: Control) -> Option<View> {
    let mut timers = lock();
    if ctl == Control::Stop {
        let old = timers.all.remove(&id)?;
        WAKE.notify_all();
        drop(timers);
        ended(app, &old);
        return None;
    }
    let entry = timers.all.get_mut(&id)?;
    let was_rest = entry.phase == Phase::Rest;
    let toast = if ctl == Control::Pause {
        entry.toggle_pause()
    } else {
        entry.restart();
        "Timer restarted"
    };
    let view = entry.view().with_toast(toast);
    wake(app, &mut timers);
    drop(timers);
    if was_rest && ctl == Control::Restart {
        breaks::end(app);
    }
    Some(view)
}

/// The break window ended the pomodoros (its button): they are gone, and the open note window is told.
pub fn stop_all_pomodoros(app: &AppHandle) {
    lock().all.retain(|_, e| e.spec.kind != Kind::Pomodoro);
    WAKE.notify_all();
    let _ = app.emit_to(super::window::WINDOW, "notes-timer", None::<String>);
}

/// Advances every timer whose phase is over; several phases may be over after a sleep: say only the last, once. Finished countdowns are removed.
fn sweep(timers: &mut Timers, now: Instant) -> Vec<Said> {
    let (mut said, mut finished) = (Vec::new(), Vec::new());
    for (id, entry) in timers.all.iter_mut() {
        let mut last = None;
        while entry.due().is_some_and(|d| d <= now) {
            let s = entry.advance();
            let done = s.done;
            last = Some(s);
            if done {
                finished.push(*id);
                break;
            }
        }
        said.extend(last);
    }
    finished.iter().for_each(|id| drop(timers.all.remove(id)));
    said
}

/// Sleeps until the next phase ends, announces it, and goes back to sleep; ends when no timer has a deadline.
fn run(app: &AppHandle) {
    let mut timers = lock();
    loop {
        let now = Instant::now();
        let said = sweep(&mut timers, now);
        if !said.is_empty() {
            drop(timers);
            said.iter().for_each(|s| announce(app, s));
            timers = lock();
            continue;
        }
        let Some(next) = timers.all.values().filter_map(Entry::due).min() else {
            timers.thread = false;
            return;
        };
        timers = WAKE.wait_timeout(timers, next.saturating_duration_since(now)).unwrap_or_else(|e| e.into_inner()).0;
    }
}

/// The alert of a phase change, by the setting for its kind: "Notification" (default) a notice and a beep, "Sound" the beep only, "Full Screen" the break window for a pomodoro's rest (a countdown treats it as a notification).
fn announce(app: &AppHandle, said: &Said) {
    let key = if said.kind == Kind::Pomodoro { "notes_pomodoro_alert" } else { "notes_countdown_alert" };
    let mode = settings::get_str(app, key).unwrap_or_default();
    let screen = said.kind == Kind::Pomodoro && mode == "Full Screen";
    let to_rest = said.kind == Kind::Pomodoro && said.phase == Phase::Rest;
    if screen && to_rest {
        breaks::begin(app, said.rest_secs, said.rounds);
    } else if said.kind == Kind::Pomodoro && said.phase == Phase::Focus {
        breaks::end(app);
    }
    if mode != "Sound" && !(screen && to_rest) {
        let prefs = Prefs::load(app);
        overlay::notice(&said.text, prefs.overlay_top, prefs.theme());
    }
    platform::beep();
    let _ = app.emit_to(super::window::WINDOW, "notes-timer", Some(&said.text));
    // the open window shows a pill; with none open there is nobody to tell
}
