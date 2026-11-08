//! File: guard.rs
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

/// Installs the panic hook and finds the crash log. Input: app handle; call once at start, before any thread of Cue's own runs.
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

/// Writes a panic to `crash.log`. Input: the panic info. Returns nothing; only a literal message and the location are kept.
fn record(info: &PanicHookInfo) {
    let Some(path) = LOG.get() else { return };
    let thread = std::thread::current();
    crate::log::append(
        path,
        &format!(
            "thread {:?}: panicked at {}: {}
{}
",
            thread.name().unwrap_or("?"),
            info.location().map_or_else(|| "?".to_string(), ToString::to_string),
            info.payload().downcast_ref::<&str>().copied().unwrap_or("(message withheld)"),
            Backtrace::force_capture()
        ),
    );
}

/// Starts a named thread (inputs: name, stack size, body; returns Err when the thread cannot start) with a small stack whose body is run under `catch_unwind`: a panic is logged by the hook, the thread ends, the user is told, and nothing else is affected.
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

/// Runs one step of a long-lived loop (a key event, a message). Input: the step. Returns its value, or None after a panic, which is logged and the loop carries on with the next one.
pub fn contain<T>(step: impl FnOnce() -> T) -> Option<T> {
    panic::catch_unwind(AssertUnwindSafe(step)).ok()
}

/// Clears the flag it holds when dropped, so a flow that panics or returns early cannot leave its shortcut disabled.
pub struct Release(pub &'static AtomicBool);

impl Drop for Release {
    /// Clears the flag. Returns nothing.
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}
