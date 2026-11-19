//! File: tray/tests.rs
//! Purpose: Protect the settings window from being built twice at once, and the claim from sticking after a failure.
//! Contents: a test of the build claim: refused while held, free after release, free after a panic.

use super::{begin_build, BUILDING};
use std::sync::atomic::Ordering;

/// A second claim is refused while the first is held, and works again after the first ends by return or by panic.
#[test]
fn only_one_build_at_a_time_and_a_panic_frees_it() {
    let first = begin_build().expect("free at start");
    assert!(begin_build().is_none(), "a second build must not start");
    drop(first);
    assert!(begin_build().is_some(), "released on drop");
    let panicked = std::panic::catch_unwind(|| {
        let _claim = begin_build().expect("free again");
        panic!("build failed");
    });
    assert!(panicked.is_err());
    assert!(!BUILDING.load(Ordering::Acquire), "the claim stayed set after the panic");
}
