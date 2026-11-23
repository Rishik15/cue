//! File: retry.rs
//! Purpose: The one paced, deadline-bound retry of a clipboard attempt.
//! Contents: retry_until — run an attempt until it succeeds or the deadline passes, pausing 20 ms doubling to 100 ms between tries.

use std::time::{Duration, Instant};

/// The first pause between two attempts.
const FIRST_PAUSE: Duration = Duration::from_millis(20);
/// The longest pause between two attempts.
const LONGEST_PAUSE: Duration = Duration::from_millis(100);

/// Calls `attempt` until it returns Ok (returned at once, even an `Ok(false)`) or `deadline` has passed (the last Err is returned). Between tries it sleeps 20 ms, doubling to 100 ms, never past the deadline.
/// shortcut: every Err is retried because errors are plain Strings; classify permanent failures (out of memory) if they ever show up, a typed error enum.
pub fn retry_until<T>(deadline: Instant, mut attempt: impl FnMut() -> Result<T, String>) -> Result<T, String> {
    let mut pause = FIRST_PAUSE;
    loop {
        let result = attempt();
        let now = Instant::now();
        if result.is_ok() || now >= deadline {
            return result;
        }
        std::thread::sleep(pause.min(deadline - now));
        pause = (pause * 2).min(LONGEST_PAUSE);
    }
}

#[cfg(test)]
mod tests {
    use super::retry_until;
    use std::time::{Duration, Instant};

    /// A closure that fails N times then succeeds is called N+1 times and its Ok returned.
    #[test]
    fn retries_until_ok() {
        let mut calls = 0;
        let out = retry_until(Instant::now() + Duration::from_secs(2), || {
            calls += 1;
            if calls <= 3 {
                Err("busy".to_string())
            } else {
                Ok(calls)
            }
        });
        assert_eq!(out, Ok(4));
    }

    /// An always-failing closure stops at the deadline with its last Err, after a bounded number of paced calls.
    #[test]
    fn gives_up_at_the_deadline() {
        let began = Instant::now();
        let mut calls = 0;
        let out: Result<(), String> = retry_until(began + Duration::from_millis(100), || {
            calls += 1;
            Err(format!("fail {calls}"))
        });
        assert_eq!(out, Err(format!("fail {calls}")));
        assert!((2..=8).contains(&calls), "calls: {calls}");
        assert!(began.elapsed() < Duration::from_millis(400));
    }

    /// An Ok is returned from the first call without sleeping, and an `Ok(false)` is not retried.
    #[test]
    fn ok_returns_at_once() {
        let began = Instant::now();
        let mut calls = 0;
        let out = retry_until(began + Duration::from_secs(5), || {
            calls += 1;
            Ok(false)
        });
        assert_eq!((out, calls), (Ok(false), 1));
        assert!(began.elapsed() < Duration::from_millis(15));
    }
}
