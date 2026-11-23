//! File: cleanup.rs
//! Purpose: Delete directories that are still locked while Cue runs (the WebView2 profile) once the process has exited.
//! Contents: remove_dirs_after_exit — starts `cue.exe --remove-data <pid> <dirs>` hidden, no shell; run_job — that helper: takes the mutex `Local\CueCleanup`, waits for the pid to exit, deletes the folders; wait_for_pending_cleanup — a new start waits (at most ten seconds) for a helper that is still deleting.

use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use windows::core::w;
use windows::Win32::Foundation::{CloseHandle, WAIT_TIMEOUT};
use windows::Win32::System::Threading::{
    CreateMutexW, OpenMutexW, OpenProcess, ReleaseMutex, WaitForSingleObject, PROCESS_SYNCHRONIZE, SYNCHRONIZATION_SYNCHRONIZE,
};

use crate::platform::cleanup_job::{parse, remove_all};

/// Process creation flag: no console window for the helper.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// How long the helper waits for Cue to exit before it gives up and keeps the data.
const EXIT_WAIT_MS: u32 = 60_000;

/// How long a new Cue waits for a helper that is still deleting.
const START_WAIT_MS: u32 = 10_000;

/// Starts `cue.exe --remove-data <this pid> <dirs>` hidden, which deletes `dirs` once this process has exited, and returns at once. Err with a line for the user when the helper cannot be started, so the caller can say the folders will stay.
pub fn remove_dirs_after_exit(dirs: &[PathBuf]) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| format!("Cue could not find its own program to start the cleanup: {e}"))?;
    Command::new(exe)
        .arg("--remove-data")
        .arg(std::process::id().to_string())
        .args(dirs)
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Cue could not start the cleanup that removes its folders: {e}"))
}

/// The helper behind `--remove-data`. Takes the mutex first, waits for the Cue process to exit (a pid that cannot be opened is already gone), then deletes the folders. Input: the process arguments. Returns None when they are not a cleanup command, else the exit code: 0 when nothing is left, 1 when Cue never exited (nothing is deleted) or a folder stayed. Safety: every handle opened here is closed before returning.
pub fn run_job(args: &[String]) -> Option<i32> {
    let job = parse(args)?;
    let mutex = unsafe { CreateMutexW(None, true, w!("Local\\CueCleanup")) }.ok();
    let code =
        if wait_for_exit(job.pid) && remove_all(&job.dirs, Duration::from_millis(250), Duration::from_secs(10)).is_empty() { 0 } else { 1 };
    if let Some(mutex) = mutex {
        unsafe {
            let _ = ReleaseMutex(mutex);
            let _ = CloseHandle(mutex);
        }
    }
    Some(code)
}

/// Waits up to a minute for process `pid` to end. Returns false only when it is still running after that. Safety: the process handle is closed before returning.
fn wait_for_exit(pid: u32) -> bool {
    unsafe {
        let Ok(process) = OpenProcess(PROCESS_SYNCHRONIZE, false, pid) else { return true };
        let waited = WaitForSingleObject(process, EXIT_WAIT_MS);
        let _ = CloseHandle(process);
        waited != WAIT_TIMEOUT
    }
}

/// First thing a normal start does: when a cleanup helper holds `Local\CueCleanup`, waits for it, at most ten seconds (an abandoned mutex also ends the wait); returns at once when there is none. Safety: the mutex handle is released and closed before returning.
pub fn wait_for_pending_cleanup() {
    unsafe {
        let Ok(mutex) = OpenMutexW(SYNCHRONIZATION_SYNCHRONIZE, false, w!("Local\\CueCleanup")) else { return };
        if WaitForSingleObject(mutex, START_WAIT_MS) != WAIT_TIMEOUT {
            let _ = ReleaseMutex(mutex);
        }
        let _: Result<(), _> = CloseHandle(mutex);
    }
}
