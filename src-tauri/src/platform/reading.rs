//! File: reading.rs
//! Purpose: The platform-neutral vocabulary of reading text from a picture: what a read returns (the text as recognised, and a separate repaired version) and the handle that stops a read that takes too long.
//! Contents: Reading — raw and repaired text plus how many lines could not be read; Stop — cancel a running read from another thread (arm, cancel, stopped).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

/// What reading a picture gave. The raw text is what the reader recognised, lines joined by paragraph breaks and nothing else changed; the repaired text is a guess at look-alike letters and screen hyphenation, offered as a separate choice because a guess can be wrong (`HCl`, `sha1sum`, `c0de`, `well-` and `known`).
pub struct Reading {
    pub raw: String,
    pub repaired: String,
    /// Lines the reader found but whose words could not be extracted; they are missing from both texts.
    pub skipped: usize,
}

/// Lets one thread cancel the read another thread runs. The reader arms it with the call that cancels its pending operation.
pub struct Stop {
    stopped: AtomicBool,
    cancel: Mutex<Option<Box<dyn FnOnce() + Send>>>,
}

impl Stop {
    /// A stop that has not been asked.
    pub fn new() -> Stop {
        Stop { stopped: AtomicBool::new(false), cancel: Mutex::new(None) }
    }

    /// Registers what cancels the read. When the stop was already asked, it runs at once.
    pub fn arm(&self, cancel: impl FnOnce() + Send + 'static) {
        let mut slot = self.cancel.lock().unwrap_or_else(|e| e.into_inner());
        if self.stopped.load(Ordering::Acquire) {
            drop(slot);
            return cancel();
        }
        *slot = Some(Box::new(cancel));
    }

    /// Asks the read to stop: runs the registered cancel once, now or when it is registered.
    pub fn cancel(&self) {
        self.stopped.store(true, Ordering::Release);
        let armed = self.cancel.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some(cancel) = armed {
            cancel();
        }
    }

    /// Whether a stop was asked.
    pub fn stopped(&self) -> bool {
        self.stopped.load(Ordering::Acquire)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use super::Stop;

    /// A cancel registered before the stop runs once when it is asked, and asking twice does not run it twice.
    #[test]
    fn an_armed_cancel_runs_once() {
        let (stop, calls) = (Stop::new(), Arc::new(AtomicUsize::new(0)));
        let seen = calls.clone();
        stop.arm(move || {
            seen.fetch_add(1, Ordering::SeqCst);
        });
        assert!(!stop.stopped());
        stop.cancel();
        stop.cancel();
        assert!(stop.stopped());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    /// A cancel registered after the stop was asked runs at once, so a read that starts late is still stopped.
    #[test]
    fn a_cancel_armed_after_the_stop_runs_at_once() {
        let (stop, calls) = (Stop::new(), Arc::new(AtomicUsize::new(0)));
        stop.cancel();
        let seen = calls.clone();
        stop.arm(move || {
            seen.fetch_add(1, Ordering::SeqCst);
        });
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}
