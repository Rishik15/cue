//! File: text/wait.rs
//! Purpose: Wait for a result that another thread produces, and give up the moment the caller says it is no longer wanted, so a model request nobody wants any more does not hold the request lock until its network timeout.
//! Contents: wait_unless — the wait.

use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::Duration;

/// Waits for a value on `rx`, asking `unwanted` every `poll`. Returns the value, or None when `unwanted` said so or the sender is gone.
pub fn wait_unless<T>(rx: &Receiver<T>, poll: Duration, unwanted: impl Fn() -> bool) -> Option<T> {
    loop {
        match rx.recv_timeout(poll) {
            Ok(value) => return Some(value),
            Err(RecvTimeoutError::Disconnected) => return None,
            Err(RecvTimeoutError::Timeout) if unwanted() => return None,
            Err(RecvTimeoutError::Timeout) => {}
        }
    }
}
