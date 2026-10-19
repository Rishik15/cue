//! Purpose: Timers for notes. A note whose first line is `timer ...` (see `spec`) runs a stopwatch, a countdown or a pomodoro that goes on while the window is closed: Rust keeps the deadline, one thread sleeps until the next one is due, says so (a notice and a beep) and ends when nothing is left to wait for.
//! Contents: set — start, keep, restart or stop the timer of a note, and report its state; View — what the page draws; the thread (`run`) is started with the first timer and ends with the last. Timers live in memory only.

mod spec;

use std::collections::HashMap;
use std::sync::{Condvar, LazyLock, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::guard;
use crate::platform::{self, overlay};
use crate::prefs::Prefs;
use spec::{Clock, Kind, Spec};

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Run,
    Focus,
    Rest,
    Done,
}

struct Entry {
    spec: Spec,
    /// The first line as typed (trimmed): the same line leaves a running timer alone, another one restarts it.
    line: String,
    phase: Phase,
    /// When the current phase began.
    began: Instant,
}

impl Entry {
    fn length(&self) -> Option<Duration> {
        match self.phase {
            Phase::Focus | Phase::Run if self.spec.kind != Kind::Stopwatch => Some(Duration::from_secs(self.spec.secs)),
            Phase::Rest => Some(Duration::from_secs(self.spec.rest)),
            _ => None,
        }
    }

    fn due(&self) -> Option<Instant> {
        self.length().map(|l| self.began + l)
    }

    /// The phase ended: a countdown is done, a pomodoro goes from focus to rest and back.
    fn advance(&mut self) -> String {
        let end = self.due().unwrap_or(self.began);
        self.began = end; // from the deadline, not from now, so a late wake-up does not stretch the next phase
        let name = if self.spec.name.is_empty() { String::new() } else { format!(": {}", self.spec.name) };
        match self.phase {
            Phase::Focus => {
                self.phase = Phase::Rest;
                format!("Focus done, time to rest{name}")
            }
            Phase::Rest => {
                self.phase = Phase::Focus;
                format!("Rest over, back to focus{name}")
            }
            _ => {
                self.phase = Phase::Done;
                format!("Timer done{name}")
            }
        }
    }

    fn view(&self) -> View {
        let elapsed = if self.phase == Phase::Done { self.length_secs() } else { self.began.elapsed().as_millis() as u64 };
        let total = if self.phase == Phase::Done { self.length_secs() } else { self.length().map_or(0, |l| l.as_millis() as u64) };
        let phase = match self.phase {
            Phase::Run => "run",
            Phase::Focus => "focus",
            Phase::Rest => "rest",
            Phase::Done => "done",
        };
        View { phase, name: self.spec.name.clone(), elapsed_ms: elapsed, total_ms: total }
    }

    fn length_secs(&self) -> u64 {
        self.spec.secs * 1000
    }
}

/// What the page draws: the phase, the timer's name, how far into the phase it is and how long the phase lasts (0 for a stopwatch).
#[derive(Serialize)]
pub struct View {
    phase: &'static str,
    name: String,
    elapsed_ms: u64,
    total_ms: u64,
}

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

/// The timer of note `id` for its first line. `None` stops it. A line that is a timer starts one only when `start` (the cursor left the line); the same line as the running timer changes nothing, another line replaces it.
pub fn set(app: &AppHandle, id: i64, line: Option<&str>, start: bool) -> Option<View> {
    let clock = platform::local_time();
    let parsed = line.and_then(|l| {
        spec::parse(l, Clock { hour: clock.hour, minute: clock.minute, second: clock.second }).map(|s| (l.trim().to_string(), s))
    });
    let mut timers = lock();
    if let (Some(entry), Some((text, _))) = (timers.all.get(&id), &parsed) {
        if entry.line == *text {
            return Some(entry.view());
        }
    }
    timers.all.remove(&id);
    let (text, spec) = parsed.filter(|_| start)?;
    let phase = if spec.kind == Kind::Pomodoro { Phase::Focus } else { Phase::Run };
    let entry = Entry { spec, line: text, phase, began: Instant::now() };
    let view = entry.view();
    timers.all.insert(id, entry);
    if !timers.thread {
        timers.thread = true;
        let handle = app.clone();
        if guard::spawn("note-timers", 128 * 1024, move || run(&handle)).is_err() {
            timers.thread = false;
        }
    }
    WAKE.notify_all();
    Some(view)
}

/// Sleeps until the next phase ends, announces it, and goes back to sleep; ends when no timer has a deadline.
fn run(app: &AppHandle) {
    let mut timers = lock();
    loop {
        let now = Instant::now();
        let mut said = Vec::new();
        for entry in timers.all.values_mut() {
            while entry.due().is_some_and(|d| d <= now) {
                said.push(entry.advance());
            }
        }
        if !said.is_empty() {
            drop(timers);
            said.iter().for_each(|text| announce(app, text));
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

fn announce(app: &AppHandle, text: &str) {
    let prefs = Prefs::load(app);
    overlay::notice(text, prefs.overlay_top, prefs.theme());
    platform::beep();
    let _ = app.emit_to(super::window::WINDOW, "notes-timer", ()); // the open window redraws; with none open there is nobody to tell
}
