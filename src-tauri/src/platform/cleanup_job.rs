//! File: cleanup_job.rs
//! Purpose: The pure part of "Remove all data": read the `--remove-data <pid> <dir>...` command line and delete the folders, with no shell and no Win32.
//! Contents: Job — the pid to wait for and the folders to delete; parse — validate the command line; remove_all — delete each folder, trying again while something still holds a file.

use std::io::ErrorKind;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// What `cue.exe --remove-data` was asked to do: wait for process `pid` to end, then delete `dirs`.
pub(crate) struct Job {
    pub pid: u32,
    pub dirs: Vec<PathBuf>,
}

/// Reads the process arguments (program name first). Returns the job when they are exactly `--remove-data <pid> <dir>...` with a numeric pid and at least one folder, each an absolute path with a parent (never a drive root or empty); None for anything else.
pub(crate) fn parse(args: &[String]) -> Option<Job> {
    let [_, flag, pid, dirs @ ..] = args else { return None };
    if flag != "--remove-data" || dirs.is_empty() {
        return None;
    }
    let dirs: Vec<PathBuf> = dirs.iter().map(PathBuf::from).collect();
    if !dirs.iter().all(|d| d.is_absolute() && d.parent().is_some()) {
        return None;
    }
    Some(Job { pid: pid.parse().ok()?, dirs })
}

/// Deletes each folder, trying again every `retry` until `give_up` has passed since the start (WebView2 children and scanners let go of files late). A folder that is already gone counts as deleted. Returns the folders that are still present.
pub(crate) fn remove_all(dirs: &[PathBuf], retry: Duration, give_up: Duration) -> Vec<PathBuf> {
    let start = Instant::now();
    dirs.iter()
        .filter(|dir| loop {
            match std::fs::remove_dir_all(dir) {
                Ok(()) => break false,
                Err(e) if e.kind() == ErrorKind::NotFound => break false,
                Err(_) if start.elapsed() >= give_up => break true,
                Err(_) => std::thread::sleep(retry),
            }
        })
        .cloned()
        .collect()
}
