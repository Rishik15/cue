//! File: model_download/child.rs
//! Purpose: Run a child process (the unpacker) as part of a download so that Cancel and Quit kill it at once.
//! Contents: run_child — spawn, register a kill with the download's cancel object, wait.

use std::io;
use std::process::{Child, Command, ExitStatus};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use super::state::Ctl;

/// How often a running child is checked for exit; only while an unpack runs.
const POLL: Duration = Duration::from_millis(25);

/// The child, locked; a panic elsewhere must not lock everyone out.
fn held(child: &Mutex<Child>) -> MutexGuard<'_, Child> {
    child.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Runs `command` to the end and returns how it exited. Cancelling `ctl` kills it at once (also when the cancel came before it started), so the caller sees a failed exit and checks `ctl.is_cancelled()`.
pub fn run_child(command: &mut Command, ctl: &Ctl) -> io::Result<ExitStatus> {
    let child = Arc::new(Mutex::new(command.spawn()?));
    let killer = child.clone();
    let id = ctl.register(Box::new(move || {
        let _ = held(&killer).kill();
    }));
    let status = loop {
        match held(&child).try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) => {}
            Err(e) => break Err(e),
        }
        std::thread::sleep(POLL);
    };
    ctl.unregister(id);
    status
}
