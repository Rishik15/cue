//! File: model_download/tests/child.rs
//! Purpose: Protect that Cancel kills an unpacker child at once, in any state of its run (Windows only: the child is `cmd.exe`).
//! Contents: scenario tests of `run_child`.

use std::process::Command;
use std::time::{Duration, Instant};

use crate::model_download::run_child;
use crate::model_download::state::{Manager, Sizes};

/// A child that would run for 30 s is gone within a few seconds of the cancel, and the download sees a failed exit it can tell from a failure by the flag.
#[cfg(windows)]
#[test]
fn cancel_kills_a_running_child_at_once() {
    let m: &'static Manager = Box::leak(Box::new(Manager::new()));
    let job = m.admit("a", "speech", Sizes { total: 1, claims: vec![] }, &|| u64::MAX).expect("job");
    let ctl = job.ctl.clone();
    let runner = std::thread::spawn(move || {
        let mut command = Command::new("cmd");
        command.args(["/c", "ping -n 30 127.0.0.1 > nul"]);
        let started = Instant::now();
        let status = run_child(&mut command, &ctl).expect("child ran");
        (status.success(), started.elapsed())
    });
    std::thread::sleep(Duration::from_millis(300));
    m.cancel("a", job.generation);
    let (success, elapsed) = runner.join().expect("runner");
    assert!(!success && elapsed < Duration::from_secs(5), "killed after {elapsed:?}");
}

/// A cancel that came before the child started kills it as it starts.
#[cfg(windows)]
#[test]
fn a_child_started_after_the_cancel_is_killed_at_once() {
    let m: &'static Manager = Box::leak(Box::new(Manager::new()));
    let job = m.admit("a", "speech", Sizes { total: 1, claims: vec![] }, &|| u64::MAX).expect("job");
    m.cancel("a", job.generation);
    let mut command = Command::new("cmd");
    command.args(["/c", "ping -n 30 127.0.0.1 > nul"]);
    let started = Instant::now();
    let status = run_child(&mut command, &job.ctl).expect("child ran");
    assert!(!status.success() && started.elapsed() < Duration::from_secs(5));
}
