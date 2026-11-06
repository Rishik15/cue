//! Keyboard listener for streaming raw key events
//!
//! This module provides a `KeyboardListener` that streams all keyboard events,
//! useful for implementing "record hotkey" UI flows.
//!
//! # Platform Notes
//!
//! - **macOS**: Uses CGEventTap. Requires accessibility permissions.
//! - **Windows**: Uses low-level keyboard hooks. Clean thread shutdown.
//! - **Linux**: Reads evdev devices directly (Wayland, X11, and console
//!   alike). Requires read access to `/dev/input` (`input` group).
//!   Blocking grabs keyboards exclusively and re-injects non-blocked
//!   events through uinput, so it additionally requires write access to
//!   `/dev/uinput`. Clean thread shutdown.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use crate::error::{Error, Result};
use crate::types::KeyEvent;

pub use crate::platform::state::BlockingHotkeys;

/// Platform-agnostic Keyboard Listener
///
/// Streams all keyboard events. Can optionally block events that match
/// registered hotkeys.
pub struct KeyboardListener {
    event_receiver: Receiver<KeyEvent>,
    _thread_handle: Option<JoinHandle<()>>,
    running: Arc<AtomicBool>,
    blocking_hotkeys: Option<BlockingHotkeys>,
    /// Backend-specific wakeup invoked on Drop after `running` is cleared.
    /// macOS parks its listener thread in a CFRunLoop with no periodic
    /// wakeups (see run_event_tap), so Drop must stop that loop explicitly.
    stop_wakeup: Option<Box<dyn Fn() + Send>>,
    /// cue patch: what only a recording listener has (Windows).
    recording: Option<Recording>,
}

/// cue patch: the controls of a recording listener: the hook thread, its arm answers, the capture gate and the lock/resume flag.
struct Recording {
    thread_id: u32,
    arm_ack: Receiver<u64>,
    epoch: Arc<AtomicU64>,
    gate: Arc<AtomicBool>,
    interrupted: Arc<AtomicBool>,
}

impl KeyboardListener {
    /// Create a new KeyboardListener (non-blocking mode)
    ///
    /// Events are observed but not blocked. Use this for "record hotkey" UI flows.
    ///
    /// On macOS, this will check for accessibility permissions and fail if not granted.
    pub fn new() -> Result<Self> {
        Self::new_internal(None, false, false)
    }

    /// Create a new KeyboardListener with blocking support
    ///
    /// Events matching hotkeys in the provided set will be blocked from reaching
    /// other applications. The set can be modified after creation to add/remove
    /// hotkeys dynamically.
    ///
    /// Note: On Linux this grabs keyboards exclusively and re-injects
    /// non-blocked events through uinput, so it requires write access to
    /// `/dev/uinput` (and fails with an actionable error without it).
    /// Mouse-button hotkeys are detected but not blocked — pointer devices
    /// are never grabbed (same behavior as Windows).
    pub fn new_with_blocking(blocking_hotkeys: BlockingHotkeys) -> Result<Self> {
        Self::new_internal(Some(blocking_hotkeys), false, false)
    }

    /// cue patch: a listener for a few seconds (a shortcut recording): its own hook, normal priority, a 10 ms loop (the way upstream handy-keys and Handy run one), and it swallows every key so
    /// the keys pressed to be recorded do nothing else (Win does not open the Start menu).
    #[cfg(target_os = "windows")]
    pub fn new_recording() -> Result<Self> {
        Self::new_internal(None, true, false)
    }

    /// cue patch: a recording listener that lives for the whole run, resting (it passes every key and costs nothing) until `arm`. A hook made on the spot hears nothing for its first moments; a resident one is ready.
    #[cfg(target_os = "windows")]
    pub fn new_resident_recording() -> Result<Self> {
        Self::new_internal(None, false, true)
    }

    fn new_internal(blocking_hotkeys: Option<BlockingHotkeys>, poll: bool, resident: bool) -> Result<Self> {
        #[cfg(target_os = "macos")]
        {
            use crate::platform::macos::listener;
            let state = listener::spawn(blocking_hotkeys)?;
            let run_loop = state.run_loop;
            Ok(KeyboardListener {
                event_receiver: state.event_receiver,
                _thread_handle: state.thread_handle,
                running: state.running,
                blocking_hotkeys: state.blocking_hotkeys,
                stop_wakeup: Some(Box::new(move || run_loop.stop())),
                recording: None,
            })
        }

        #[cfg(target_os = "windows")]
        {
            use crate::platform::windows::listener;
            let state = listener::spawn(blocking_hotkeys, poll, resident)?;
            let thread_id = state.thread_id;
            let recording = match (state.gate, state.arm_ack) {
                (Some(gate), Some(arm_ack)) => Some(Recording { thread_id, arm_ack, epoch: state.epoch, gate, interrupted: state.interrupted }),
                _ => None,
            };
            Ok(KeyboardListener {
                event_receiver: state.event_receiver,
                _thread_handle: state.thread_handle,
                running: state.running,
                blocking_hotkeys: state.blocking_hotkeys,
                stop_wakeup: Some(Box::new(move || listener::wake(thread_id))),
                recording,
            })
        }

        #[cfg(target_os = "linux")]
        {
            use crate::platform::linux::listener;
            let state = listener::spawn(blocking_hotkeys)?;
            Ok(KeyboardListener {
                event_receiver: state.event_receiver,
                _thread_handle: state.thread_handle,
                running: state.running,
                blocking_hotkeys: state.blocking_hotkeys,
                stop_wakeup: None,
                recording: None,
            })
        }
    }

    /// cue patch: starts a resident recording listener capturing: drops what it heard before, asks its thread to clear its key state and take in the keys held now, and waits for that to be done.
    /// Returns the token of this arm, which `release_keyboard` needs, or `None` when the thread did not answer in time (the arm is then cancelled and the listener rests).
    #[cfg(target_os = "windows")]
    pub fn arm(&self) -> Option<u64> {
        let rec = self.recording.as_ref()?;
        while self.event_receiver.try_recv().is_ok() {}
        while rec.arm_ack.try_recv().is_ok() {}
        rec.interrupted.store(false, Ordering::Release);
        let token = rec.epoch.fetch_add(1, Ordering::AcqRel) + 1;
        let end = std::time::Instant::now() + Duration::from_millis(500);
        let answered = crate::platform::windows::listener::request_arm(rec.thread_id, token)
            && std::iter::from_fn(|| end.checked_duration_since(std::time::Instant::now()).and_then(|left| rec.arm_ack.recv_timeout(left).ok())).any(|t| t == token);
        if answered {
            return Some(token);
        }
        self.release_keyboard(token);
        None
    }

    /// cue patch: stops swallowing keys at the next key, before the recording is dropped, if `token` is still the current arm. A release by an older recording, or a late arm request it cancels, does nothing to a newer one.
    pub fn release_keyboard(&self, token: u64) {
        let Some(rec) = &self.recording else { return };
        if rec.epoch.compare_exchange(token, token + 1, Ordering::AcqRel, Ordering::Acquire).is_ok() {
            rec.gate.store(false, Ordering::Release);
        }
    }

    /// cue patch: whether a lock, unlock, connect or resume happened since this recording listener started.
    pub fn interrupted(&self) -> bool {
        self.recording.as_ref().is_some_and(|rec| rec.interrupted.load(Ordering::Acquire))
    }

    /// Get a reference to the blocking hotkeys set (if blocking is enabled)
    pub fn blocking_hotkeys(&self) -> Option<&BlockingHotkeys> {
        self.blocking_hotkeys.as_ref()
    }

    /// Blocking receive for key events
    ///
    /// Blocks until a key event is received or the listener stops.
    pub fn recv(&self) -> Result<KeyEvent> {
        self.event_receiver
            .recv()
            .map_err(|_| Error::EventLoopNotRunning)
    }

    /// Blocking receive with timeout
    ///
    /// Blocks until a key event is received, the timeout expires, or the listener stops.
    pub fn recv_timeout(&self, timeout: Duration) -> Result<KeyEvent> {
        self.event_receiver
            .recv_timeout(timeout)
            .map_err(|e| match e {
                std::sync::mpsc::RecvTimeoutError::Timeout => Error::Timeout,
                std::sync::mpsc::RecvTimeoutError::Disconnected => Error::EventLoopNotRunning,
            })
    }

    /// Non-blocking receive for key events
    ///
    /// Returns `Some(event)` if an event is available, `None` otherwise.
    pub fn try_recv(&self) -> Option<KeyEvent> {
        match self.event_receiver.try_recv() {
            Ok(event) => Some(event),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => None,
        }
    }
}

impl Drop for KeyboardListener {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);

        // Wake the listener thread if the backend parks it (macOS stops the
        // tap thread's CFRunLoop). Windows/Linux event loops re-check
        // `running` at least every ~100ms on their own, so the join below is
        // short and shutdown is clean on all platforms.
        if let Some(wake) = &self.stop_wakeup {
            wake();
        }
        if let Some(handle) = self._thread_handle.take() {
            let _ = handle.join();
        }
    }
}
