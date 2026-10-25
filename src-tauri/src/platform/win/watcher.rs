//! Purpose: The life of a watcher thread (a hidden window that Windows tells about keys or the clipboard), owned in one place so no watcher can be started twice or leak. Both watchers exist only while a feature needs them.
//! Contents: Lifecycle — start (serialised: a second call waits for the first and finds it running), stop (ends the thread and waits until it is gone); Ready — what the thread calls once its window exists and once it has ended.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{mpsc, Mutex};
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{PostThreadMessageW, WM_QUIT};

const START_WITHIN: Duration = Duration::from_secs(2);
const STOP_WITHIN: Duration = Duration::from_millis(500);

pub struct Lifecycle {
    /// Held for the whole of a start or a stop, so they never overlap.
    gate: Mutex<()>,
    /// The watcher thread's id while it runs, 0 when it does not.
    thread: AtomicU32,
}

/// Handed to the watcher thread's body.
pub struct Ready {
    answer: mpsc::Sender<Result<(), String>>,
    lifecycle: &'static Lifecycle,
}

impl Ready {
    /// The window exists: `start` returns. False when `start` has given up waiting; the body must then clean up and return without a message loop.
    pub fn running(&self) -> bool {
        self.lifecycle.thread.store(unsafe { GetCurrentThreadId() }, Ordering::Release);
        let heard = self.answer.send(Ok(())).is_ok();
        if !heard {
            self.lifecycle.thread.store(0, Ordering::Release);
        }
        heard
    }

    pub fn failed(&self, why: String) {
        let _ = self.answer.send(Err(why));
    }

    /// The message loop is over and the window is gone.
    pub fn ended(&self) {
        self.lifecycle.thread.store(0, Ordering::Release);
    }
}

impl Lifecycle {
    pub const fn new() -> Self {
        Lifecycle { gate: Mutex::new(()), thread: AtomicU32::new(0) }
    }

    /// Starts the thread unless it already runs. `what` names the thing watched, for the error text.
    pub fn start(&'static self, name: &str, stack: usize, what: &str, body: impl FnOnce(Ready) + Send + 'static) -> Result<(), String> {
        let _gate = self.gate.lock().unwrap_or_else(|e| e.into_inner());
        if self.thread.load(Ordering::Acquire) != 0 {
            return Ok(());
        }
        let (answer, heard) = mpsc::channel();
        crate::guard::spawn(name, stack, move || body(Ready { answer, lifecycle: self }))
            .map_err(|e| format!("Could not start watching the {what}: {e}"))?;
        heard.recv_timeout(START_WITHIN).map_err(|_| format!("The {what} watcher did not start."))?
    }

    /// Ends the thread and waits until its window is gone, so a watcher started right after finds a clean slate.
    pub fn stop(&self) {
        let _gate = self.gate.lock().unwrap_or_else(|e| e.into_inner());
        let thread = self.thread.load(Ordering::Acquire);
        if thread == 0 {
            return;
        }
        // SAFETY: wakes the watcher's message loop so it ends and removes its window.
        let _ = unsafe { PostThreadMessageW(thread, WM_QUIT, WPARAM(0), LPARAM(0)) };
        let until = Instant::now() + STOP_WITHIN;
        while self.thread.load(Ordering::Acquire) != 0 && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}
