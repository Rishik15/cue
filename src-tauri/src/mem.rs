//! File: mem.rs
//! Purpose: Return freed memory to the OS after the app goes idle (window closed, mic released), never while calls keep arriving.
//! Contents: trim_soon — debounced working-set trim, one second after the last call; State and settle — request, final check and trim under one lock; quiet_left — the pure wait arithmetic.

use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

/// Debounce state; one lock makes "register a request" and "retire the worker" indivisible.
struct State {
    /// A worker thread is waiting to trim.
    pending: bool,
    /// Milliseconds since `start()` of the latest request.
    last: u64,
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
}

/// Locks the debounce state, ignoring poisoning (the state is always consistent).
fn state() -> MutexGuard<'static, State> {
    lock(&STATE)
}

/// Locks `m`, ignoring poisoning.
fn lock(m: &Mutex<State>) -> MutexGuard<'_, State> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// One worker check under the lock at `now`: returns the milliseconds still to wait, or runs `trim` and retires the worker before releasing the lock, so no request can register between the final quiet check and the trim.
fn settle(m: &Mutex<State>, now: u64, trim: impl FnOnce()) -> Option<u64> {
    let mut st = lock(m);
    match quiet_left(st.last, now, QUIET_MS) {
        0 => {
            trim();
            st.pending = false;
            None
        }
        left => Some(left),
    }
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
        while let Some(ms) = settle(&STATE, now_ms(), crate::platform::trim_working_set) {
            std::thread::sleep(Duration::from_millis(ms));
        }
    });
    if spawned.is_err() {
        state().pending = false;
    }
}

#[cfg(test)]
mod tests {
    use super::{lock, quiet_left, settle, State};
    use std::sync::Mutex;

    /// The remaining quiet time counts down, is 0 once passed, and a `last_ms` ahead of `now_ms` never underflows.
    #[test]
    fn quiet_left_counts_down_without_underflow() {
        assert_eq!(quiet_left(100, 400, 1000), 700);
        assert_eq!(quiet_left(100, 1100, 1000), 0);
        assert_eq!(quiet_left(100, 5000, 1000), 0);
        assert_eq!(quiet_left(500, 100, 1000), 1000);
    }

    /// A request before the final check pushes the trim back; the trim runs with the lock held, so a request cannot register mid-trim; one after retirement spawns a new worker.
    #[test]
    fn trim_runs_under_the_lock_and_retires_the_worker() {
        let m = Mutex::new(State { pending: false, last: 0 });
        assert!(lock(&m).request(0));
        assert!(!lock(&m).request(999));
        assert_eq!(settle(&m, 1000, || panic!("trimmed early")), Some(999));
        let mut trimmed = false;
        assert_eq!(
            settle(&m, 1999, || {
                assert!(m.try_lock().is_err());
                trimmed = true;
            }),
            None
        );
        assert!(trimmed);
        assert!(lock(&m).request(1999));
    }
}
