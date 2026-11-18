//! File: notes/timer/mod.rs
//! Purpose: Timers for notes. A line `timer ...` (see `spec`) in a note starts a stopwatch, a countdown or a pomodoro that goes on while the window is closed: Rust keeps the deadline, one thread sleeps until the next one is due, says so (a notice, a sound or a full-screen break, per the alert settings) and ends when nothing is left to wait for. One timer per note.
//! Contents: set — start, keep or report the timer of a note; control — pause, restart or stop it; stop_all_pomodoros — the break window's end button; the thread (`run`) is started when a timer with a deadline starts or resumes and ends with the last; wake and unstartable — thread start and its failure cleanup; announce — the alert of a phase change, dropped when its timer was stopped, restarted or replaced meanwhile (`live`). Timers live in memory only.

mod clock;
mod entry;
mod spec;

use std::collections::HashMap;
use std::sync::{Condvar, LazyLock, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter};

use super::breaks::{self, Owner};
use crate::platform::{self, overlay};
use crate::prefs::Prefs;
use crate::sound::{self, Sound};
use crate::{guard, settings, status};
use clock::Clock;
pub use entry::View;
use entry::{Entry, Phase, Said};
pub use spec::Control;
use spec::Kind;

/// Every note's timer, and whether the thread that watches them is alive.
struct Timers {
    all: HashMap<i64, Entry>,
    /// The thread is alive (decided under this lock, so a timer set as it ends is never left without one).
    thread: bool,
}

static TIMERS: LazyLock<Mutex<Timers>> = LazyLock::new(|| Mutex::new(Timers { all: HashMap::new(), thread: false }));
static WAKE: Condvar = Condvar::new();
/// How long before a deadline the thread stops sleeping and spins (Windows sleeps are coarse, ~15 ms).
const SPIN: Duration = Duration::from_millis(20);
/// How long before a deadline the audio device is woken.
const WARM: Duration = Duration::from_secs(1);

/// The timers, locked; a lock poisoned by a panic is used as it is.
fn lock() -> MutexGuard<'static, Timers> {
    TIMERS.lock().unwrap_or_else(|e| e.into_inner())
}

/// Starts the thread that watches the timers. Inputs: the app handle. Returns the spawn result.
fn spawn_thread(app: &AppHandle) -> std::io::Result<()> {
    let handle = app.clone();
    guard::spawn("note-timers", 128 * 1024, move || run(&handle))
}

/// Wakes the thread, starting it with `start` when it is not alive. Returns false when the thread could not be started: nothing would alert.
fn wake(timers: &mut Timers, start: impl FnOnce() -> std::io::Result<()>) -> bool {
    if !timers.thread {
        timers.thread = true;
        if start().is_err() {
            timers.thread = false;
            return false;
        }
    }
    WAKE.notify_all();
    true
}

/// The owners whose breaks must close when the timer `gone` (just removed) could not start: its own and `previous`, the owner it replaced or restarted, whose break would otherwise stay open with no timer.
fn orphans(gone: Option<Entry>, previous: Option<Owner>) -> Vec<Owner> {
    gone.map(|e| e.owner).into_iter().chain(previous).collect()
}

/// The timer of note `id` could not get its thread: it is removed, the user is told, and the breaks of it and of the owner it replaced (`previous`) go. Takes the lock held by the caller. Returns None, the view of a timer that is not running.
fn unstartable(app: &AppHandle, mut timers: MutexGuard<'static, Timers>, id: i64, previous: Option<Owner>) -> Option<View> {
    let gone = timers.all.remove(&id);
    drop(timers);
    orphans(gone, previous).into_iter().for_each(|o| breaks::cancel(app, o));
    status::report(app, "Could not start the timer");
    None
}

/// Is the timer that said `owner` still the one of its note? A countdown that ran out (`done`) has already left the table, and still counts.
fn live(timers: &Timers, owner: Owner, done: bool) -> bool {
    timers.all.get(&owner.note).map_or(done, |e| e.owner == owner)
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
    let entry = Entry::new(id, spec, text);
    let view = entry.view().with_toast(entry.started());
    let old = timers.all.insert(id, entry);
    if !wake(&mut timers, || spawn_thread(app)) {
        return unstartable(app, timers, id, old.map(|o| o.owner));
    }
    drop(timers);
    old.iter().for_each(|o| breaks::cancel(app, o.owner));
    Some(view)
}

/// Pause (toggles), restart or stop the timer of note `id`. A stopped timer is gone: None.
pub fn control(app: &AppHandle, id: i64, ctl: Control) -> Option<View> {
    let mut timers = lock();
    if ctl == Control::Stop {
        let old = timers.all.remove(&id)?;
        WAKE.notify_all();
        drop(timers);
        breaks::cancel(app, old.owner);
        return None;
    }
    let entry = timers.all.get_mut(&id)?;
    let before = entry.owner;
    let toast = if ctl == Control::Pause {
        entry.toggle_pause()
    } else {
        entry.restart();
        "Timer restarted"
    };
    let view = entry.view().with_toast(toast);
    if !wake(&mut timers, || spawn_thread(app)) {
        return unstartable(app, timers, id, Some(before));
    }
    drop(timers);
    if ctl == Control::Restart {
        breaks::cancel(app, before);
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

/// Sleeps until the next phase ends, announces it, and goes back to sleep; ends when no timer has a deadline. A silent sound shortly before the alert wakes the audio device, so the real one is not late (a device asleep for minutes would be if warmed at the start); a sleep on Windows ends up to ~15 ms late, so it sleeps to just before the deadline and spins the rest. shortcut: ~15 ms of spin per phase end, a high-resolution timer if that ever shows.
fn run(app: &AppHandle) {
    let mut timers = lock();
    let mut warmed = None;
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
        let left = next.saturating_duration_since(now);
        if left > WARM {
            timers = WAKE.wait_timeout(timers, left - WARM).unwrap_or_else(|e| e.into_inner()).0;
            continue;
        }
        if warmed != Some(next) {
            warmed = Some(next);
            sound::play(Sound::Timer, 0);
        }
        if left > SPIN {
            timers = WAKE.wait_timeout(timers, left - SPIN).unwrap_or_else(|e| e.into_inner()).0;
            continue;
        }
        drop(timers);
        while Instant::now() < next {
            std::hint::spin_loop();
        }
        timers = lock();
    }
}

/// The alert of a phase change (`play` does not wait, so the sound starts first and the window after it; the open note shows a pill), by the setting for its kind: "Notification" (default) and "Sound" the tone at the Alert Volume, the pill in an open note and the notice outside when none is open ("Sound": beep only), "Full Screen" the break window for a pomodoro's rest (a countdown treats it as a notification).
fn announce(app: &AppHandle, said: &Said) {
    if !live(&lock(), said.owner, said.done) {
        return;
    }
    sound::play(Sound::Timer, settings::get_u64("notes_alert_volume", 70).min(100) as u8);
    let key = if said.kind == Kind::Pomodoro { "notes_pomodoro_alert" } else { "notes_countdown_alert" };
    let mode = settings::get_str(key).unwrap_or_default();
    let screen = said.kind == Kind::Pomodoro && mode == "Full Screen";
    let to_rest = said.kind == Kind::Pomodoro && said.phase == Phase::Rest;
    if screen && to_rest {
        breaks::begin(app, said.owner, said.rest_secs, said.rounds, || live(&lock(), said.owner, false));
    } else if said.kind == Kind::Pomodoro && said.phase == Phase::Focus {
        breaks::end(app, said.owner);
    }
    if super::window::exists(app) {
        let _ = app.emit_to(super::window::WINDOW, "notes-timer", Some(&said.text));
    } else if mode != "Sound" && !(screen && to_rest) {
        let prefs = Prefs::load();
        overlay::notice(&said.text, prefs.overlay_top, prefs.theme());
    }
}

#[cfg(test)]
mod tests;
