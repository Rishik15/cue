//! File: mem.rs
//! Purpose: Return freed memory to the OS after the app goes idle (window closed, mic released).
//! Contents: trim_soon — schedule one working-set trim ~1 s later, coalescing bursts.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

static PENDING: AtomicBool = AtomicBool::new(false);

/// Schedules one working-set trim about a second later; calls during the wait share it. Returns nothing.
pub fn trim_soon() {
    if PENDING.swap(true, Ordering::AcqRel) {
        return;
    }
    let spawned = crate::guard::spawn("trim", 64 * 1024, || {
        std::thread::sleep(Duration::from_secs(1));
        PENDING.store(false, Ordering::Release);
        crate::platform::trim_working_set();
    });
    if spawned.is_err() {
        PENDING.store(false, Ordering::Release);
    }
}
