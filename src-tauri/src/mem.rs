//! Purpose: Return freed memory to the OS after the app goes idle (window closed, mic released).
//! Contents: trim_soon — schedule one working-set trim ~1 s later, coalescing bursts.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

static PENDING: AtomicBool = AtomicBool::new(false);

pub fn trim_soon() {
    if PENDING.swap(true, Ordering::AcqRel) {
        return;
    }
    let spawned = std::thread::Builder::new().name("trim".into()).stack_size(64 * 1024).spawn(|| {
        std::thread::sleep(Duration::from_secs(1));
        PENDING.store(false, Ordering::Release);
        crate::platform::trim_working_set();
    });
    if spawned.is_err() {
        PENDING.store(false, Ordering::Release); // otherwise no trim would ever be scheduled again
    }
}
