//! File: tray/tests.rs
//! Purpose: Protect the settings window open and close from sleeps, hangs and double builds.
//! Contents: tests of the close tracker (woken by the destroy, never hangs) and of the open lock (two opens build once, a close is waited out inside the lock) and of the pending page (the latest request survives until taken).

use super::{open_with, request_page, take_start_page, Closing, START_PAGE};
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
                    || {},
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

/// An open queued behind a close and a slow build still waits for the close inside the lock, so it sees the window the first open built, never a stale state.
#[test]
fn queued_open_waits_for_close_inside_the_lock() {
    let window = AtomicBool::new(false);
    let order = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for name in ["first", "second"] {
            let (window, order) = (&window, &order);
            s.spawn(move || {
                open_with(
                    || order.lock().unwrap().push(format!("{name} waits")),
                    || window.load(Ordering::SeqCst).then_some(()),
                    |()| order.lock().unwrap().push(format!("{name} focuses")),
                    || {
                        std::thread::sleep(Duration::from_millis(100));
                        window.store(true, Ordering::SeqCst);
                        order.lock().unwrap().push(format!("{name} builds"));
                    },
                );
            });
        }
    });
    let order = order.into_inner().unwrap();
    assert_eq!(order.len(), 4, "{order:?}");
    assert!(
        order[0].ends_with("waits") && order[1].ends_with("builds") && order[2].ends_with("waits") && order[3].ends_with("focuses"),
        "{order:?}"
    );
}

/// Two requests for different pages arrive before the page starts listening: the later page is kept until taken, and only once.
#[test]
fn latest_page_waits_until_the_window_takes_it() {
    *START_PAGE.lock().unwrap() = None;
    request_page(Some("about"));
    request_page(Some("words"));
    request_page(None);
    assert_eq!(take_start_page(), Some("words"));
    assert_eq!(take_start_page(), None);
}
