//! File: mem.rs
//! Purpose: Return freed memory to the OS after the app goes idle (window closed, mic released), never while calls keep arriving.
//! Contents: trim_soon — debounced working-set trim, one second after the last call; State — request/retire under one lock; quiet_left — the pure wait arithmetic.

use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

/// Debounce state; one lock makes "register a request" and "retire the worker" indivisible.
struct State {
    /// A worker thread is waiting to trim.
    pending: bool,
    /// Milliseconds since `start()` of the latest request.
    last: u64,
}

/// What the worker does next.
#[derive(Debug, PartialEq)]
enum Step {
    Wait(u64),
    Trim,
}

static STATE: Mutex<State> = Mutex::new(State { pending: false, last: 0 });
/// How long no call must arrive before the trim runs.
const QUIET_MS: u64 = 1000;

impl State {
    /// Records a request at `now`; returns true when the caller must spawn the worker.
    fn request(&mut self, now: u64) -> bool {
        self.last = now;
        !std::mem::replace(&mut self.pending, true)
    }

    /// Decides the worker's next move at `now`; on `Trim` the worker is retired in the same step, so a later request spawns a fresh one.
    fn step(&mut self, now: u64) -> Step {
        match quiet_left(self.last, now, QUIET_MS) {
            0 => {
                self.pending = false;
                Step::Trim
            }
            left => Step::Wait(left),
        }
    }
}

/// Locks the debounce state, ignoring poisoning (the state is always consistent).
fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

/// The monotonic clock origin, fixed at first use.
fn start() -> Instant {
    static START: OnceLock<Instant> = OnceLock::new();
    *START.get_or_init(Instant::now)
}

/// Milliseconds on the monotonic clock.
fn now_ms() -> u64 {
    start().elapsed().as_millis() as u64
}

/// Milliseconds still to wait for `quiet_ms` of quiet since `last_ms`, given `now_ms`; 0 once it has passed (or when `last_ms` is ahead of `now_ms`).
fn quiet_left(last_ms: u64, now_ms: u64, quiet_ms: u64) -> u64 {
    quiet_ms.saturating_sub(now_ms.saturating_sub(last_ms))
}

/// Schedules one working-set trim for one second after the latest call; calls during the wait push it back and share the one thread. Returns nothing.
pub fn trim_soon() {
    if !state().request(now_ms()) {
        return;
    }
    let spawned = crate::guard::spawn("trim", 64 * 1024, || {
        loop {
            let step = state().step(now_ms());
            match step {
                Step::Wait(ms) => std::thread::sleep(Duration::from_millis(ms)),
                Step::Trim => break,
            }
        }
        crate::platform::trim_working_set();
    });
    if spawned.is_err() {
        state().pending = false;
    }
}

#[cfg(test)]
mod tests {
    use super::{quiet_left, State, Step};

    /// The remaining quiet time counts down, is 0 once passed, and a `last_ms` ahead of `now_ms` never underflows.
    #[test]
    fn quiet_left_counts_down_without_underflow() {
        assert_eq!(quiet_left(100, 400, 1000), 700);
        assert_eq!(quiet_left(100, 1100, 1000), 0);
        assert_eq!(quiet_left(100, 5000, 1000), 0);
        assert_eq!(quiet_left(500, 100, 1000), 1000);
    }

    /// A request landing just before the worker's quiet check pushes the trim back; one landing after the retiring check spawns a new worker instead of being swallowed.
    #[test]
    fn request_racing_the_final_check_is_never_lost() {
        let mut st = State { pending: false, last: 0 };
        assert!(st.request(0));
        assert!(!st.request(999));
        assert_eq!(st.step(1000), Step::Wait(999));
        assert_eq!(st.step(1999), Step::Trim);
        assert!(st.request(1999));
        assert_eq!(st.step(2000), Step::Wait(999));
    }
}
