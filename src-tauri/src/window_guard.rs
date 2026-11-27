//! File: window_guard.rs
//! Purpose: The one lifecycle guard the preview, strip and dictionary windows share: a fresh owner number per window, its native handle and in-front test, the one blur rule of ADR 0023 and the one watchdog whose expiry names the window it guards.
//! Contents: fresh — a new owner number; handle — a window's Win32 handle; in_front — whether a window is still the one in front; leaves_on_blur — the blur rule; Focus — which window has held the keyboard; send_open — the open event's one send; Watch — a one-slot watchdog that fires with its owner's number.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::lock::lock;
use crate::platform;

/// A new owner number for a window, never 0 (0 means no window). Return: the number.
pub fn fresh() -> u64 {
    static LAST: AtomicU64 = AtomicU64::new(0);
    LAST.fetch_add(1, Ordering::AcqRel) + 1
}

/// The native handle of a window. Input: the window. Return: the handle, None when it cannot be read.
#[cfg(windows)]
pub fn handle(window: &tauri::WebviewWindow) -> Option<isize> {
    window.hwnd().ok().map(|h| h.0 as isize)
}

/// The native handle of a window. Input: the window. Return: always None off Windows.
#[cfg(not(windows))]
pub fn handle(_: &tauri::WebviewWindow) -> Option<isize> {
    None
}

/// Whether the window labelled `label` is still the window in front. Input: the app and the label. Return: false when the window or its handle is gone.
pub fn in_front(app: &AppHandle, label: &str) -> bool {
    app.get_webview_window(label).and_then(|w| handle(&w)).is_some_and(platform::in_front)
}

/// The one rule of ADR 0023: losing the keyboard closes a window only when it had held it and is no longer in front. A loss while it is still in front is the keyboard moving between the window and its web page, not the user leaving. Input: whether the window held the keyboard and whether it is in front. Return: the answer.
pub fn leaves_on_blur(held: bool, in_front: bool) -> bool {
    held && !in_front
}

/// The owner number of the window that has held the keyboard, 0 for none, so a click away closes a window only after that. Keyed by owner: a new window starts without it and an older window's events cannot touch it.
pub struct Focus(AtomicU64);

impl Focus {
    /// No window has held the keyboard.
    pub const fn new() -> Self {
        Self(AtomicU64::new(0))
    }

    /// Window `owner` gained the keyboard. Input: its owner number.
    pub fn gained(&self, owner: u64) {
        self.0.store(owner, Ordering::Release);
    }

    /// Whether window `owner` has held the keyboard. Input: its owner number. Return: the answer, false for 0.
    pub fn held(&self, owner: u64) -> bool {
        owner != 0 && self.0.load(Ordering::Acquire) == owner
    }

    /// Window `owner` lost the keyboard. Input: its owner number and whether it is still in front. Return: whether it must close; the mark clears only then, so a loss while in front keeps it.
    pub fn lost(&self, owner: u64, in_front: bool) -> bool {
        let leave = leaves_on_blur(self.held(owner), in_front);
        if leave {
            let _ = self.0.compare_exchange(owner, 0, Ordering::AcqRel, Ordering::Acquire);
        }
        leave
    }
}

/// Sends a window's claimed open event and logs when it went or why not; the one send of the claim-send-unsent handshake the strip and dictionary windows share. Input: the app, the feature's name for the log, the window's label, the event, its payload and the milliseconds since the press. Return: whether it was sent; on false the caller hands the claim back (`unsent`) so it may be claimed again.
pub fn send_open(app: &AppHandle, feature: &str, label: &str, event: &str, payload: impl Serialize + Clone, at_ms: u128) -> bool {
    let sent = app.emit_to(label, event, payload);
    match &sent {
        Ok(()) => crate::log::note(&format!("{feature}: open event sent at {at_ms} ms")),
        Err(e) => crate::log::note(&format!("{feature}: the open event failed: {e}")),
    }
    sent.is_ok()
}

/// A one-slot watchdog: arming ends the previous one, and the expiry carries the owner it was armed for so the feature can check it still owns the window under its own lock.
pub struct Watch {
    slot: Mutex<Option<(u64, Sender<()>)>>,
}

impl Watch {
    /// A watchdog that is not armed.
    pub const fn new() -> Self {
        Self { slot: Mutex::new(None) }
    }

    /// Starts one small thread that calls `expire(owner)` unless it is disarmed, replaced or its watch dropped within `deadline`; an older thread wakes with a disconnect and does nothing. The slot lock is not held while `expire` runs. Input: the thread name, the owner's number, the deadline and the expiry. Return: an error when the thread cannot start.
    pub fn arm(
        &self,
        name: &'static str,
        owner: u64,
        deadline: Duration,
        expire: impl FnOnce(u64) + Send + 'static,
    ) -> std::io::Result<()> {
        let (armed, wait) = mpsc::channel::<()>();
        *lock(&self.slot) = Some((owner, armed));
        crate::guard::spawn(name, 64 * 1024, move || {
            if wait.recv_timeout(deadline) == Err(RecvTimeoutError::Timeout) {
                expire(owner);
            }
        })
    }

    /// Ends the watchdog, if one waits.
    pub fn disarm(&self) {
        lock(&self.slot).take();
    }

    /// Ends the watchdog only when it was armed for `owner`; another owner's stays. Input: the owner's number.
    pub fn disarm_if(&self, owner: u64) {
        let mut slot = lock(&self.slot);
        if slot.as_ref().is_some_and(|(o, _)| *o == owner) {
            slot.take();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{fresh, leaves_on_blur, Focus, Watch};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    /// A watch armed for `owner` with a 50 ms deadline, counting its expiries into `fired` (the owner it saw, 0 for none).
    fn armed(watch: &Watch, owner: u64, fired: &Arc<AtomicU64>) {
        let fired = fired.clone();
        watch.arm("test-watch", owner, Duration::from_millis(50), move |o| fired.store(o, Ordering::SeqCst)).unwrap();
    }

    /// Waits past the deadline of `armed`.
    fn wait() {
        std::thread::sleep(Duration::from_millis(200));
    }

    /// Only a window that held the keyboard and is no longer in front leaves.
    #[test]
    fn the_blur_rule_truth_table() {
        assert!(leaves_on_blur(true, false));
        assert!(!leaves_on_blur(true, true));
        assert!(!leaves_on_blur(false, false));
        assert!(!leaves_on_blur(false, true));
        assert_ne!(fresh(), 0);
        assert_ne!(fresh(), fresh());
    }

    /// A loss while still in front keeps the mark, so a later real loss closes; a loss with no gain never closes; a new owner starts unmarked and an older owner's loss closes nothing.
    #[test]
    fn a_loss_in_front_keeps_the_focus_mark() {
        let focus = Focus::new();
        assert!(!focus.lost(1, false));
        focus.gained(1);
        assert!(focus.held(1));
        assert!(!focus.lost(1, true));
        assert!(focus.lost(1, false));
        assert!(!focus.lost(1, false));
        focus.gained(1);
        assert!(!focus.held(2));
        assert!(!focus.lost(2, false));
        focus.gained(2);
        assert!(!focus.lost(1, false));
        assert!(focus.held(2));
        assert!(!focus.held(0));
    }

    /// An armed watch that nobody disarms fires once with its owner; it needs no visibility input.
    #[test]
    fn an_armed_watch_fires_with_its_owner() {
        let (watch, fired) = (Watch::new(), Arc::new(AtomicU64::new(0)));
        armed(&watch, 3, &fired);
        wait();
        assert_eq!(fired.load(Ordering::SeqCst), 3);
    }

    /// Disarming, or disarming for the same owner, stops the expiry; disarming for another owner leaves it armed.
    #[test]
    fn disarm_stops_the_expiry_unless_another_owners() {
        let fired = Arc::new(AtomicU64::new(0));
        let watch = Watch::new();
        armed(&watch, 1, &fired);
        watch.disarm();
        armed(&watch, 2, &fired);
        watch.disarm_if(2);
        wait();
        assert_eq!(fired.load(Ordering::SeqCst), 0);
        armed(&watch, 4, &fired);
        watch.disarm_if(5);
        wait();
        assert_eq!(fired.load(Ordering::SeqCst), 4);
    }

    /// Re-arming for a newer owner wakes the older thread without an expiry: only the newer owner fires.
    #[test]
    fn a_rearm_silences_the_older_owner() {
        let (watch, fired) = (Watch::new(), Arc::new(AtomicU64::new(0)));
        armed(&watch, 1, &fired);
        armed(&watch, 2, &fired);
        wait();
        assert_eq!(fired.load(Ordering::SeqCst), 2);
    }
}
