//! File: text/tests/wait.rs
//! Purpose: Prove that a stalled model request no longer holds its caller once the caller is not wanted.
//! Contents: tests of `wait_unless`.

use std::cell::Cell;
use std::sync::mpsc::channel;
use std::time::Duration;

use crate::text::wait::wait_unless;

/// A request that never answers is abandoned on the first poll after the caller stops being wanted.
#[test]
fn a_stalled_request_is_abandoned_when_unwanted() {
    let (_keep_open, rx) = channel::<u8>();
    let polls = Cell::new(0);
    let out = wait_unless(&rx, Duration::from_millis(1), || {
        polls.set(polls.get() + 1);
        polls.get() >= 3
    });
    assert_eq!(out, None);
    assert_eq!(polls.get(), 3);
}

/// A reply that arrives is returned, and a sender that is gone ends the wait.
#[test]
fn a_reply_is_returned_and_a_dead_sender_ends_the_wait() {
    let (tx, rx) = channel();
    tx.send(7).unwrap();
    assert_eq!(wait_unless(&rx, Duration::from_millis(1), || false), Some(7));
    drop(tx);
    assert_eq!(wait_unless(&rx, Duration::from_millis(1), || false), None);
}
