//! File: notes/tests/opening.rs
//! Purpose: Protect the shortcut after a failed opening: a panic while the note window opens must not leave it disabled.
//! Contents: a test that panics inside the opening work and then claims the flag again.

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use crate::notes::{begin_opening, OPENING};

/// After the opening work panics, the next press can claim the opening flag.
#[test]
fn a_panic_while_opening_frees_the_shortcut() {
    assert!(!OPENING.swap(true, Ordering::AcqRel));
    begin_opening(|| panic!("opening failed")).expect("thread starts");
    let give_up = Instant::now() + Duration::from_secs(10);
    while OPENING.load(Ordering::Acquire) && Instant::now() < give_up {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(!OPENING.swap(true, Ordering::AcqRel), "the flag stayed set after the panic");
    OPENING.store(false, Ordering::Release);
}
