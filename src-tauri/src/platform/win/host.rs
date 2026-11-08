//! File: host.rs
//! Purpose: The life of a native surface thread (the pill, the card): created by the first message sent to it, destroyed when the surface has nothing left to show, so an idle Cue holds no window and no thread for either. Messages sent while the thread starts are kept and delivered in order, so a caller never waits for the window. A surface that keeps failing to start is retried a few times and then left alone for a while, and the user is told once per failure episode.
//! Contents:
//! - Host — `post` (send a message, starting the thread when asked), `ready` / `failed` (the thread says its window exists or could not be made), `retire` (the thread asks to end; refused when a message is still waiting)
//! - register_class — register a window class once per process
//!
//! shortcut: a window class stays registered for the life of the process (a few bytes), because unregistering it on thread exit races the next thread's registration; unregister it explicitly if classes ever carry per-run data.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{GetLastError, ERROR_CLASS_ALREADY_EXISTS, HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    PeekMessageW, PostMessageW, RegisterClassW, MSG, PM_NOREMOVE, PM_QS_POSTMESSAGE, WM_APP, WNDCLASSW,
};

/// One window message: id, wparam, lparam.
type Msg = (u32, usize, isize);

/// Failed starts in a row after which the surface is not started again until `RETRY_AFTER` has passed.
const MAX_STRIKES: u32 = 3;
const RETRY_AFTER: Duration = Duration::from_secs(60);

/// Where the surface thread is in its life.
enum Phase {
    Off,
    Starting(Vec<Msg>),
    Running(isize),
}

/// The phase plus the failure episode: how many starts failed in a row and when the last one did.
struct State {
    phase: Phase,
    strikes: u32,
    last_failure: Option<Instant>,
}

/// A surface thread that exists only while it has work.
pub struct Host {
    name: &'static str,
    run: fn(),
    state: Mutex<State>,
}

impl Host {
    /// Describes a surface whose thread body is `run` (it must call `ready` once its window exists, `failed` if it cannot make one, and `retire` to end).
    pub const fn new(name: &'static str, run: fn()) -> Host {
        Host { name, run, state: Mutex::new(State { phase: Phase::Off, strikes: 0, last_failure: None }) }
    }

    /// Locks the state; a poisoned lock still holds a usable state.
    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Sends a message to the surface window. Input: the message and, in `start`, whether a missing thread is started for it (a hide has nothing to hide, so it does not). Return: false when the message went nowhere (no thread and none started, the thread could not start, or the surface failed `MAX_STRIKES` times in a row less than `RETRY_AFTER` ago).
    pub fn post(&self, msg: u32, wp: usize, lp: isize, start: bool) -> bool {
        let mut guard = self.lock();
        let state = &mut *guard;
        if let Phase::Running(hwnd) = &state.phase {
            return send(*hwnd, (msg, wp, lp));
        }
        if let Phase::Starting(queue) = &mut state.phase {
            queue.push((msg, wp, lp));
            return true;
        }
        let benched = state.strikes >= MAX_STRIKES && state.last_failure.is_some_and(|at| at.elapsed() < RETRY_AFTER);
        if !start || benched {
            return false;
        }
        state.phase = Phase::Starting(vec![(msg, wp, lp)]);
        if let Err(e) = crate::guard::spawn(self.name, 256 * 1024, self.run) {
            state.phase = Phase::Off;
            drop(guard);
            self.failed(&format!("could not start its thread: {e}"));
            return false;
        }
        true
    }

    /// The thread's window exists: the failure episode ends and the messages kept while it started are delivered now, in order.
    pub fn ready(&self, hwnd: HWND) {
        let mut state = self.lock();
        state.strikes = 0;
        if let Phase::Starting(queue) = std::mem::replace(&mut state.phase, Phase::Running(hwnd.0 as isize)) {
            queue.into_iter().for_each(|m| {
                send(hwnd.0 as isize, m);
            });
        }
    }

    /// The thread could not make its window and is ending: kept messages are dropped. The user is told only for the first failure of an episode, and after `MAX_STRIKES` the surface is not started again until `RETRY_AFTER` has passed.
    pub fn failed(&self, why: &str) {
        let first = {
            let mut state = self.lock();
            state.phase = Phase::Off;
            state.strikes += 1;
            state.last_failure = Some(Instant::now());
            state.strikes == 1
        };
        crate::log::note(&format!("{}: {why}", self.name));
        if first {
            crate::platform::report(&format!("Cue could not show its {} window: {why}", self.name));
        }
    }

    /// The thread asks to end because its surface is hidden. Return: true when it may destroy its window and leave; false when a posted message is still waiting for it (or the window is not this host's), in which case it keeps running. Safety: `waiting` is a valid out-parameter, and PM_QS_POSTMESSAGE keeps sent messages, which could re-enter the surface and this lock, out of the peek.
    pub fn retire(&self, hwnd: HWND) -> bool {
        let mut state = self.lock();
        if !matches!(&state.phase, Phase::Running(h) if *h == hwnd.0 as isize) {
            return false;
        }
        let mut waiting = MSG::default();
        let pending = unsafe { PeekMessageW(&mut waiting, Some(hwnd), WM_APP, WM_APP + 0x3FFF, PM_NOREMOVE | PM_QS_POSTMESSAGE) };
        if pending.as_bool() {
            return false;
        }
        state.phase = Phase::Off;
        true
    }
}

/// Registers a window class once per process. Return: true when it is registered (now or by an earlier thread of the same surface). Safety: `class` must be fully filled in and its name and window procedure live for the whole process.
pub unsafe fn register_class(class: &WNDCLASSW) -> bool {
    RegisterClassW(class) != 0 || GetLastError() == ERROR_CLASS_ALREADY_EXISTS
}

/// Posts one message to `hwnd`. Return: whether Windows accepted it. Safety: a plain post to a window handle; a window that is gone makes it fail.
fn send(hwnd: isize, (msg, wp, lp): Msg) -> bool {
    unsafe { PostMessageW(Some(HWND(hwnd as _)), msg, WPARAM(wp), LPARAM(lp)) }.is_ok()
}
