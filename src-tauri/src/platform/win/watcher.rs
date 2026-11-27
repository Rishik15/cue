//! File: watcher.rs
//! Purpose: The life of a watcher thread (a hidden window that Windows tells about keys or the clipboard), owned in one place so no watcher can be started twice or leak. Both watchers exist only while a feature needs them. The decisions (late start, failed start, stop during a start) are the pure `platform::watcher_state::Machine`; this file holds it under a short mutex and does the thread, wait and quit calls it asks for.
//! Contents:
//! - Lifecycle — start (serialised: a second call waits for the first and finds it running), stop (ends the thread and waits until it is gone)
//! - Ready — what the thread calls once its window exists and once it has ended

use crate::lock::lock;
use std::sync::{mpsc, Mutex};
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{PostThreadMessageW, WM_QUIT};

use crate::platform::watcher_state::{Effect, Event, Machine, State};

const START_WITHIN: Duration = Duration::from_secs(2);
const STOP_WITHIN: Duration = Duration::from_millis(500);

/// The state of one watcher thread: a gate that serialises start and stop, and the decision machine (which also holds the accepted thread's id).
pub struct Lifecycle {
    /// Held for the whole of a start or a stop, so they never overlap.
    gate: Mutex<()>,
    /// Which generation runs, starts or is closing; locked only for a decision, never across an OS call.
    machine: Mutex<Machine>,
}

/// Handed to the watcher thread's body.
pub struct Ready {
    answer: mpsc::Sender<Result<(), String>>,
    lifecycle: &'static Lifecycle,
    generation: u64,
}

impl Ready {
    /// The window exists: `start` returns. False when this start was given up on (timed out, or stopped meanwhile); the body must then clean up and return without a message loop. Safety: `GetCurrentThreadId` has no preconditions.
    pub fn running(&self) -> bool {
        let id = unsafe { GetCurrentThreadId() };
        let accepted = self.lifecycle.step(Event::Ready(self.generation, id)).contains(&Effect::Proceed(self.generation));
        let _ = self.answer.send(if accepted { Ok(()) } else { Err("The watcher was stopped while it started.".to_string()) });
        accepted
    }

    /// The window could not be made: `start` returns this reason. Input: the user-readable cause.
    pub fn failed(&self, why: String) {
        self.lifecycle.step(Event::StartFailed(self.generation));
        let _ = self.answer.send(Err(why));
    }

    /// The message loop is over and the window is gone.
    pub fn ended(&self) {
        self.lifecycle.step(Event::Ended(self.generation));
    }
}

impl Drop for Ready {
    /// Runs on every thread exit, panic included: reports this generation ended, so a thread that died before answering never leaves it stuck in Stopping. A repeat of `ended` or `failed` changes nothing.
    fn drop(&mut self) {
        self.lifecycle.step(Event::Ended(self.generation));
    }
}

impl Lifecycle {
    /// An idle lifecycle with no thread running.
    pub const fn new() -> Self {
        Lifecycle { gate: Mutex::new(()), machine: Mutex::new(Machine::new()) }
    }

    /// Feeds one event to the machine under its lock. Input: the event. Returns what to do.
    fn step(&self, event: Event) -> Vec<Effect> {
        lock(&self.machine).apply(event)
    }

    /// Asks the machine for a new generation. Some(generation) to spawn, None when a watcher already runs or starts; a watcher still closing is waited for once. Err when it is still closing after that.
    fn begin(&self, what: &str) -> Result<Option<u64>, String> {
        for attempt in 0..2 {
            match self.step(Event::Start).first() {
                Some(Effect::Spawn(generation)) => return Ok(Some(*generation)),
                None => return Ok(None),
                Some(_) => {}
            }
            if attempt == 0 {
                self.wait_closed(what)?;
            }
        }
        Err(format!("The {what} watcher is still closing."))
    }

    /// Starts the thread unless it already runs. `what` names the thing watched, for the error text.
    pub fn start(&'static self, name: &str, stack: usize, what: &str, body: impl FnOnce(Ready) + Send + 'static) -> Result<(), String> {
        let _gate = lock(&self.gate);
        let Some(generation) = self.begin(what)? else { return Ok(()) };
        let (answer, heard) = mpsc::channel();
        if let Err(e) = crate::guard::spawn(name, stack, move || body(Ready { answer, lifecycle: self, generation })) {
            self.step(Event::StartFailed(generation));
            return Err(format!("Could not start watching the {what}: {e}"));
        }
        match heard.recv_timeout(START_WITHIN) {
            Ok(answer) => answer,
            Err(_) if self.step(Event::StartTimedOut(generation)).contains(&Effect::Proceed(generation)) => Ok(()),
            Err(_) => Err(format!("The {what} watcher did not start.")),
        }
    }

    /// Ends the thread and waits until its window is gone, so a watcher started right after finds a clean slate. Safety: Wakes the watcher's message loop so it ends and removes its window.
    pub fn stop(&self) {
        let _gate = lock(&self.gate);
        if let Some(&Effect::Quit(generation, thread)) = self.step(Event::Stop).first() {
            if unsafe { PostThreadMessageW(thread, WM_QUIT, WPARAM(0), LPARAM(0)) }.is_err() {
                self.step(Event::QuitFailed(generation));
                return;
            }
        }
        let _ = self.wait_closed("");
    }

    /// Waits up to `STOP_WITHIN` for a closing watcher to be gone. Err when a watcher of `what` is still closing after that, so a start never reports success for a thread that is about to end.
    /// shortcut: polls every 5 ms for at most `STOP_WITHIN` (500 ms), only while a watcher stops; upgrade to a condvar woken by `Ready::ended` if stops become frequent.
    fn wait_closed(&self, what: &str) -> Result<(), String> {
        let until = Instant::now() + STOP_WITHIN;
        while self.closing() && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(5));
        }
        if self.closing() {
            return Err(format!("The {what} watcher is still closing."));
        }
        Ok(())
    }

    /// Whether a watcher is on its way out.
    fn closing(&self) -> bool {
        matches!(lock(&self.machine).state(), State::Stopping(_))
    }
}
