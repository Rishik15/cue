//! File: tray/tests.rs
//! Purpose: Protect the settings window open and close from sleeps, hangs and double builds.
//! Contents: tests of the close tracker (woken by the destroy, never hangs) and of the open lock (two opens build once).

use super::{open_with, Closing};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// A waiter returns as soon as the destroy ends the close, and a second wait finds nothing to wait for.
#[test]
fn closing_wakes_the_waiter_on_destroy() {
    let closing = Closing::new();
    closing.begin();
    let began = Instant::now();
    std::thread::scope(|s| {
        s.spawn(|| {
            std::thread::sleep(Duration::from_millis(50));
            closing.end();
        });
        assert!(closing.wait(Duration::from_secs(2)));
    });
    assert!(began.elapsed() < Duration::from_secs(1));
    assert!(!closing.wait(Duration::from_secs(2)));
}

/// With no destroy the wait ends at its limit.
#[test]
fn closing_never_hangs_without_destroy() {
    let closing = Closing::new();
    closing.begin();
    let began = Instant::now();
    assert!(closing.wait(Duration::from_millis(100)));
    let took = began.elapsed();
    assert!(took >= Duration::from_millis(90) && took < Duration::from_secs(1), "{took:?}");
}

/// Two simultaneous opens build the window once; the second focuses it.
#[test]
fn two_opens_build_once() {
    let window = AtomicBool::new(false);
    let builds = AtomicUsize::new(0);
    let focuses = AtomicUsize::new(0);
    std::thread::scope(|s| {
        for _ in 0..2 {
            s.spawn(|| {
                open_with(
                    || window.load(Ordering::SeqCst).then_some(()),
                    |()| {
                        focuses.fetch_add(1, Ordering::SeqCst);
                    },
                    || {
                        builds.fetch_add(1, Ordering::SeqCst);
                        std::thread::sleep(Duration::from_millis(100));
                        window.store(true, Ordering::SeqCst);
                    },
                );
            });
        }
    });
    assert_eq!(builds.load(Ordering::SeqCst), 1);
    assert_eq!(focuses.load(Ordering::SeqCst), 1);
}
