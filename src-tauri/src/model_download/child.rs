//! File: model_download/child.rs
//! Purpose: Run a child process (the unpacker) as part of a download so that Cancel and Quit kill it at once.
//! Contents: run_child — spawn, register a kill with the download's cancel object, block until it exits.

use std::io;
use std::process::{Command, ExitStatus};
use std::sync::{Arc, Mutex, PoisonError};

use super::ctl::Ctl;

/// Runs `command` to the end and returns how it exited. Cancelling `ctl` kills it at once from the cancel interrupt (also when the cancel came before it started), so the caller sees a failed exit and checks `ctl.is_cancelled()`. A gate makes the kill and the child's release exclusive, so a kill that is running while the child exits never acts on a closed handle.
pub fn run_child(command: &mut Command, ctl: &Ctl) -> io::Result<ExitStatus> {
    let mut child = command.spawn()?;
    let kill = crate::platform::process_killer(&child);
    let gate = Arc::new(Mutex::new(true));
    let open = gate.clone();
    let id = ctl.register(Box::new(move || {
        if *open.lock().unwrap_or_else(PoisonError::into_inner) {
            kill();
        }
    }));
    let status = child.wait();
    ctl.unregister(id);
    *gate.lock().unwrap_or_else(PoisonError::into_inner) = false;
    status
}
