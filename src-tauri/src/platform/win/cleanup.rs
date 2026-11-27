//! File: cleanup.rs
//! Purpose: Delete directories that are still locked while Cue runs (the WebView2 profile) once the process has exited.
//! Contents: remove_dirs_after_exit — starts `cue.exe --remove-data <pid> <dirs>` hidden, no shell, and returns only when the helper holds the mutex; run_job — that helper: owns the mutex `Local\CueCleanup`, tells the parent it is ready, waits for the pid to exit, deletes the folders; wait_for_pending_cleanup — a new start waits (bounded by the helper's own limits) until a helper that is still deleting is done; acquire / release / wait_for_exit / is_gone / wait_ready — the Win32 steps, each checking the exact wait result.

use std::os::windows::io::AsRawHandle;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::time::Duration;

use windows::core::{HRESULT, HSTRING};
use windows::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, ERROR_INVALID_PARAMETER, HANDLE, WAIT_ABANDONED, WAIT_OBJECT_0,
};
use windows::Win32::System::Threading::{
    CreateEventW, CreateMutexW, OpenEventW, OpenMutexW, OpenProcess, ReleaseMutex, SetEvent, WaitForMultipleObjects, WaitForSingleObject,
    EVENT_MODIFY_STATE, PROCESS_SYNCHRONIZE, SYNCHRONIZATION_SYNCHRONIZE,
};

use crate::platform::cleanup_job::{parse, remove_all};

/// Process creation flag: no console window for the helper.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// How long the helper waits for Cue to exit before it gives up and keeps the data.
const EXIT_WAIT_MS: u32 = 60_000;

/// How long the helper waits for the mutex when another helper still holds it.
const LOCK_WAIT_MS: u32 = 90_000;

/// How long the exiting Cue waits for its helper to hold the mutex.
const READY_WAIT_MS: u32 = 10_000;

/// How long a new start waits for a helper that holds the mutex: its exit wait plus its 10 s of delete retries and slack, so a live helper always finishes first and a stuck one cannot hang the start.
const CLEANUP_WAIT_MS: u32 = EXIT_WAIT_MS + 15_000;

/// Name of the mutex a helper owns for as long as it waits for Cue and deletes.
fn mutex_name() -> HSTRING {
    HSTRING::from("Local\\CueCleanup")
}

/// Name of the event with which the helper for process `pid` says it owns the mutex.
fn ready_name(pid: u32) -> HSTRING {
    HSTRING::from(format!("Local\\CueCleanupReady{pid}"))
}

/// Starts `cue.exe --remove-data <this pid> <dirs>` hidden, which deletes `dirs` once this process has exited, and returns once that helper owns the mutex `Local\CueCleanup` (so a fast relaunch finds it). Err with a line for the user when the helper cannot be started or does not get ready (it is stopped then), so the caller can say the folders will stay.
pub fn remove_dirs_after_exit(dirs: &[PathBuf]) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| format!("Cue could not find its own program to start the cleanup: {e}"))?;
    let pid = std::process::id();
    let ready = unsafe { CreateEventW(None, true, false, &ready_name(pid)) }
        .map_err(|e| format!("Cue could not prepare the cleanup that removes its folders: {e}"))?;
    let started = Command::new(exe).arg("--remove-data").arg(pid.to_string()).args(dirs).creation_flags(CREATE_NO_WINDOW).spawn();
    let result = match started {
        Ok(mut child) => wait_ready(ready, &mut child, READY_WAIT_MS),
        Err(e) => Err(format!("Cue could not start the cleanup that removes its folders: {e}")),
    };
    unsafe {
        let _ = CloseHandle(ready);
    }
    result
}

/// Waits until `ready` is set, the helper `child` ends, or `ms` pass. Ok only when `ready` was set; otherwise stops the helper (so it deletes nothing) and says the folders stay.
fn wait_ready(ready: HANDLE, child: &mut Child, ms: u32) -> Result<(), String> {
    let handles = [ready, HANDLE(child.as_raw_handle())];
    if unsafe { WaitForMultipleObjects(&handles, false, ms) } == WAIT_OBJECT_0 {
        return Ok(());
    }
    let _ = child.kill();
    let _ = child.wait();
    Err("Cue's cleanup helper did not get ready, so the folders stay.".to_string())
}

/// The helper behind `--remove-data`. Owns the mutex first (abort when it cannot), tells the parent it is ready, waits for the Cue process to exit (a pid that no longer exists is already gone), then deletes the folders. Input: the process arguments. Returns None when they are not a cleanup command, else the exit code: 0 when nothing is left, 1 when the mutex was not won, Cue never exited (nothing is deleted) or a folder stayed.
pub fn run_job(args: &[String]) -> Option<i32> {
    let job = parse(args)?;
    let Some(mutex) = acquire(&mutex_name(), LOCK_WAIT_MS) else { return Some(1) };
    signal_ready(job.pid);
    let done =
        wait_for_exit(job.pid, EXIT_WAIT_MS) && remove_all(&job.dirs, Duration::from_millis(250), Duration::from_secs(10)).is_empty();
    release(mutex);
    Some(if done { 0 } else { 1 })
}

/// Takes ownership of the named mutex `name`: created already owned when absent (never published unowned), else waited for at most `ms` while another owner has it. Returns its handle only when this thread owns it now (an abandoned mutex counts as won); None otherwise, with the handle closed.
fn acquire(name: &HSTRING, ms: u32) -> Option<HANDLE> {
    unsafe {
        let mutex = CreateMutexW(None, true, name).ok()?;
        if GetLastError() != ERROR_ALREADY_EXISTS {
            return Some(mutex);
        }
        let waited = WaitForSingleObject(mutex, ms);
        if waited == WAIT_OBJECT_0 || waited == WAIT_ABANDONED {
            return Some(mutex);
        }
        let _ = CloseHandle(mutex);
        None
    }
}

/// Gives up the mutex taken by `acquire` and closes its handle.
fn release(mutex: HANDLE) {
    unsafe {
        let _ = ReleaseMutex(mutex);
        let _ = CloseHandle(mutex);
    }
}

/// Sets the event with which the parent `pid` waits for its helper. Does nothing when the parent is not waiting.
fn signal_ready(pid: u32) {
    unsafe {
        let Ok(ready) = OpenEventW(EVENT_MODIFY_STATE, false, &ready_name(pid)) else { return };
        let _ = SetEvent(ready);
        let _ = CloseHandle(ready);
    }
}

/// Waits up to `ms` for process `pid` to end. Returns true only when it ended or no process has that pid (`is_gone`); false when it still runs, cannot be opened for another reason (access denied) or the wait failed. Safety: the process handle is closed before returning.
fn wait_for_exit(pid: u32, ms: u32) -> bool {
    unsafe {
        let process = match OpenProcess(PROCESS_SYNCHRONIZE, false, pid) {
            Ok(process) => process,
            Err(e) => return is_gone(e.code()),
        };
        let waited = WaitForSingleObject(process, ms);
        let _ = CloseHandle(process);
        waited == WAIT_OBJECT_0
    }
}

/// Whether an `OpenProcess` failure means the process is gone: only `ERROR_INVALID_PARAMETER` (no such pid) does; any other error (access denied) means it may still run. Input: the error code. Returns the verdict.
fn is_gone(code: HRESULT) -> bool {
    code == ERROR_INVALID_PARAMETER.to_hresult()
}

/// Runs at the start of a normal launch and again once the launch is the primary instance: when a cleanup helper owns `Local\\CueCleanup`, waits until it is done (an abandoned mutex also ends the wait), so a start never overlaps a deletion; returns at once when there is none. The wait is capped at `CLEANUP_WAIT_MS`; past it the start goes on and the log says so. The second call closes the window where a launch passed the first check, then became primary after the helper took the mutex.
pub fn wait_for_pending_cleanup() {
    if !wait_for_cleanup(&mutex_name(), CLEANUP_WAIT_MS) {
        crate::log::note(&format!("cleanup: a helper still held the mutex after {CLEANUP_WAIT_MS} ms, starting anyway"));
    }
}

/// Waits up to `ms` until the mutex `name` is free. Returns true when it was free, became free or does not exist; false when the wait timed out or failed. Safety: the mutex handle is released and closed before returning.
fn wait_for_cleanup(name: &HSTRING, ms: u32) -> bool {
    unsafe {
        let Ok(mutex) = OpenMutexW(SYNCHRONIZATION_SYNCHRONIZE, false, name) else { return true };
        let waited = WaitForSingleObject(mutex, ms);
        let won = waited == WAIT_OBJECT_0 || waited == WAIT_ABANDONED;
        if won {
            let _ = ReleaseMutex(mutex);
        }
        let _ = CloseHandle(mutex);
        won
    }
}

#[cfg(test)]
mod tests {
    use std::process::{Command, Stdio};

    use windows::core::HSTRING;
    use windows::Win32::Foundation::{CloseHandle, ERROR_INVALID_PARAMETER, E_ACCESSDENIED};
    use windows::Win32::System::Threading::CreateEventW;

    use super::{acquire, is_gone, ready_name, release, signal_ready, wait_for_cleanup, wait_for_exit, wait_ready};

    /// A mutex name no other test or Cue uses.
    fn unique(tag: &str) -> HSTRING {
        HSTRING::from(format!("Local\\CueCleanupTest{tag}{}", std::process::id()))
    }

    /// A child that lives a few seconds.
    fn slow_child() -> std::process::Child {
        Command::new("cmd").args(["/C", "ping -n 4 127.0.0.1"]).stdout(Stdio::null()).spawn().unwrap()
    }

    /// A mutex that already exists and is held by another thread is not owned by a second `acquire` (the old `bInitialOwner` check said it was); once released it is.
    #[test]
    fn acquire_waits_for_the_current_owner() {
        let name = unique("owner");
        let first = acquire(&name, 0).expect("free mutex");
        let n = name.clone();
        assert!(!std::thread::spawn(move || acquire(&n, 50).is_some()).join().unwrap());
        release(first);
        let n = name.clone();
        assert!(std::thread::spawn(move || acquire(&n, 1000).map(release).is_some()).join().unwrap());
    }

    /// A mutex whose owner thread ended without releasing it (abandoned) is still won by the next `acquire`.
    #[test]
    fn acquire_takes_an_abandoned_mutex() {
        let name = unique("abandoned");
        let n = name.clone();
        std::thread::spawn(move || assert!(acquire(&n, 0).is_some())).join().unwrap();
        let n = name.clone();
        assert!(std::thread::spawn(move || acquire(&n, 1000).map(release).is_some()).join().unwrap());
    }

    /// A launch that waits for cleanup stays blocked while the helper owns the mutex and goes on once it is released; with no helper it returns at once; a helper that holds on past the limit ends the wait with false.
    #[test]
    fn wait_for_cleanup_blocks_until_the_helper_is_done() {
        let name = unique("wait");
        assert!(wait_for_cleanup(&name, 0));
        let helper = acquire(&name, 0).expect("free mutex");
        let n = name.clone();
        assert!(!std::thread::spawn(move || wait_for_cleanup(&n, 50)).join().unwrap());
        let n = name.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(wait_for_cleanup(&n, 10_000));
        });
        assert!(rx.recv_timeout(std::time::Duration::from_millis(200)).is_err());
        release(helper);
        assert_eq!(rx.recv_timeout(std::time::Duration::from_secs(5)), Ok(true));
    }

    /// Only "no such process" counts as gone; access denied (an elevated or protected process) means it may still run.
    #[test]
    fn only_a_missing_pid_is_gone() {
        assert!(is_gone(ERROR_INVALID_PARAMETER.to_hresult()));
        assert!(!is_gone(E_ACCESSDENIED));
    }

    /// Only a real exit counts: a running process is not "exited" after the wait, a finished one is, a pid that cannot be opened is.
    #[test]
    fn wait_for_exit_needs_a_real_exit() {
        assert!(!wait_for_exit(std::process::id(), 50));
        let mut done = Command::new("cmd").args(["/C", "exit"]).spawn().unwrap();
        let pid = done.id();
        done.wait().unwrap();
        assert!(wait_for_exit(pid, 1000));
    }

    /// The parent proceeds only after the helper signalled; a helper that ends or never signals fails the handshake and is stopped.
    #[test]
    fn handshake_needs_the_helper_signal() {
        let pid = std::process::id() ^ 0x5a5a_0000;
        let ready = unsafe { CreateEventW(None, true, false, &ready_name(pid)) }.unwrap();
        let mut silent = slow_child();
        assert!(wait_ready(ready, &mut silent, 100).is_err());
        let mut gone = Command::new("cmd").args(["/C", "exit"]).spawn().unwrap();
        assert!(wait_ready(ready, &mut gone, 3000).is_err());
        signal_ready(pid);
        let mut alive = slow_child();
        assert!(wait_ready(ready, &mut alive, 3000).is_ok());
        let _ = alive.kill();
        unsafe {
            let _ = CloseHandle(ready);
        }
    }
}
