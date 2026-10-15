//! Purpose: Keeps a bug in one feature from taking the whole app down. A panic is written to `crash.log` in the data folder with a backtrace, a thread body that panics is caught
//! (the app and its other threads carry on) and the user is told, and the busy flags of the shortcut flows are released however a flow ends.
//! Contents: install — the panic hook and the log; spawn — a named thread whose body is caught; contain — one loop step that may panic; Release — clears a busy flag when dropped.
//! The release profile unwinds (not aborts) so that this works; the cost is a slightly larger executable.

use std::backtrace::Backtrace;
use std::panic::{self, AssertUnwindSafe, PanicHookInfo};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

use tauri::{AppHandle, Manager};

use crate::status;

static APP: OnceLock<AppHandle> = OnceLock::new();
static LOG: OnceLock<PathBuf> = OnceLock::new();

/// Call once at start, before any thread of Cue's own runs.
pub fn install(app: &AppHandle) {
    let _ = APP.set(app.clone());
    if let Ok(dir) = app.path().app_data_dir() {
        let _ = std::fs::create_dir_all(&dir);
        let _ = LOG.set(dir.join("crash.log"));
    }
    let previous = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        record(info);
        previous(info);
    }));
}

fn record(info: &PanicHookInfo) {
    let Some(path) = LOG.get() else { return };
    let thread = std::thread::current();
    crate::log::append(
        path,
        &format!(
            "thread {:?}: {info}
{}
",
            thread.name().unwrap_or("?"),
            Backtrace::force_capture()
        ),
    );
}

/// Starts a named thread with a small stack whose body is run under `catch_unwind`: a panic is logged by the hook, the thread ends, the user is told, and nothing else is affected.
pub fn spawn(name: &str, stack: usize, body: impl FnOnce() + Send + 'static) -> std::io::Result<()> {
    let label = name.to_string();
    std::thread::Builder::new().name(label.clone()).stack_size(stack).spawn(move || {
        if panic::catch_unwind(AssertUnwindSafe(body)).is_err() {
            if let Some(app) = APP.get() {
                status::report(app, &format!("Cue hit a problem in {label} and recovered. Details are in crash.log in Cue's data folder."));
            }
        }
    })?;
    Ok(())
}

/// Runs one step of a long-lived loop (a key event, a message): a panic in it is logged and the loop carries on with the next one.
pub fn contain<T>(step: impl FnOnce() -> T) -> Option<T> {
    panic::catch_unwind(AssertUnwindSafe(step)).ok()
}

/// Clears a flag when dropped, so a flow that panics or returns early cannot leave its shortcut disabled.
pub struct Release(pub &'static AtomicBool);

impl Drop for Release {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn a_panicking_thread_body_is_contained_and_the_flag_is_released() {
        static BUSY: AtomicBool = AtomicBool::new(true);
        let (tx, rx) = mpsc::channel();
        spawn("test-panic", 256 * 1024, move || {
            let _release = Release(&BUSY);
            let _ = tx.send(());
            panic!("expected by the test");
        })
        .unwrap();
        rx.recv_timeout(Duration::from_secs(5)).unwrap();
        for _ in 0..200 {
            if !BUSY.load(Ordering::Acquire) {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("the flag was not released");
    }
}
