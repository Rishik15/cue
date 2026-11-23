//! File: mem.rs
//! Purpose: Return freed memory to the OS after the app goes idle (window closed, mic released), never while calls keep arriving.
//! Contents: trim_soon — debounced working-set trim, one second after the last call; quiet_left — the pure wait arithmetic.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

static PENDING: AtomicBool = AtomicBool::new(false);
/// Milliseconds since `start()` of the latest `trim_soon` call.
static LAST: AtomicU64 = AtomicU64::new(0);
/// How long no call must arrive before the trim runs.
const QUIET_MS: u64 = 1000;

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
    LAST.store(now_ms(), Ordering::Release);
    if PENDING.swap(true, Ordering::AcqRel) {
        return;
    }
    let spawned = crate::guard::spawn("trim", 64 * 1024, || {
        loop {
            let left = quiet_left(LAST.load(Ordering::Acquire), now_ms(), QUIET_MS);
            if left == 0 {
                break;
            }
            std::thread::sleep(Duration::from_millis(left));
        }
        PENDING.store(false, Ordering::Release);
        crate::platform::trim_working_set();
    });
    if spawned.is_err() {
        PENDING.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::quiet_left;

    /// The remaining quiet time counts down, is 0 once passed, and a `last_ms` ahead of `now_ms` never underflows.
    #[test]
    fn quiet_left_counts_down_without_underflow() {
        assert_eq!(quiet_left(100, 400, 1000), 700);
        assert_eq!(quiet_left(100, 1100, 1000), 0);
        assert_eq!(quiet_left(100, 5000, 1000), 0);
        assert_eq!(quiet_left(500, 100, 1000), 1000);
    }
}
