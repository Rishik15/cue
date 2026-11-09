//! File: model_download/tests/worker.rs
//! Purpose: Protect a whole download job against fakes: success, a failure in each phase, a cancel in each phase (connect, read, verify, install), resume, a wrong checksum and a full disk, each with what the windows were told and what the table remembers.
//! Contents: scenario tests of `job::run` through `Rig`.

use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::channel;
use std::sync::Arc;

use super::fakes::{await_entered, hang, plan_of, resumed, whole, Event, FakeNet, Rig, StallingBody};
use super::server::{data, file_of};
use crate::model_download::net::Fetched;
use crate::model_download::state::Outcome;
use crate::model_download::transfer::Stop;
use crate::model_download::After;

const SIZE: usize = 200_000;

/// An installer that records that it ran.
fn installer(ran: &Arc<AtomicBool>) -> After {
    let ran = ran.clone();
    Box::new(move |_| {
        ran.store(true, Ordering::SeqCst);
        Ok(())
    })
}

/// A download whose bytes arrive whole is verified, renamed, installed and announced as succeeded, with the percent going up and the phase change announced.
#[test]
fn a_download_succeeds_end_to_end() {
    let bytes = data(SIZE);
    let body = bytes.clone();
    let rig = Rig::new(FakeNet::new(move |_, _, _| Ok(Some(whole(&body)))), u64::MAX);
    let dir = tempfile::tempdir().expect("dir");
    let ran = Arc::new(AtomicBool::new(false));
    rig.run("m", plan_of("https://x/m.bin", file_of("m.bin", &bytes), dir.path()), installer(&ran));
    assert_eq!(fs::read(dir.path().join("m.bin")).expect("file"), bytes);
    assert!(ran.load(Ordering::SeqCst));
    let finished = rig.sink.finished().expect("announced");
    assert_eq!((finished.outcome, finished.error), (Outcome::Succeeded, None));
    let events = rig.sink.events();
    assert!(events.iter().any(|e| matches!(e, Event::Progress(p) if p.done == SIZE as u64 && p.total == SIZE as u64)));
    let changes = events.iter().filter(|e| matches!(e, Event::Changed(_))).count();
    assert_eq!(changes, 1, "only the install phase was announced: a fresh file has nothing to verify");
    assert!(rig.env.manager.view().job("m").is_none());
    assert_eq!(rig.env.manager.view().last("m").map(|l| l.outcome), Some(Outcome::Succeeded));
}

/// A server that refuses fails the download with its message, installs nothing, and the table remembers the failure.
#[test]
fn a_failure_while_fetching_is_reported_and_remembered() {
    let rig = Rig::new(FakeNet::new(|_, _, _| Err(Stop::Failed("The download is not available right now (error 404).".into()))), u64::MAX);
    let dir = tempfile::tempdir().expect("dir");
    let ran = Arc::new(AtomicBool::new(false));
    rig.run("m", plan_of("https://x/m.bin", file_of("m.bin", &data(SIZE)), dir.path()), installer(&ran));
    let finished = rig.sink.finished().expect("announced");
    assert_eq!(finished.outcome, Outcome::Failed);
    assert_eq!(finished.error.as_deref(), Some("The download is not available right now (error 404)."));
    assert!(!ran.load(Ordering::SeqCst));
    assert!(!dir.path().join("m.bin").exists());
    assert_eq!(rig.env.manager.view().last("m").and_then(|l| l.error.clone()).as_deref(), finished.error.as_deref());
}

/// Bytes that do not match the pinned hash never get the real name, the partial is removed, and nothing is installed.
#[test]
fn a_wrong_checksum_fails_verification_and_installs_nothing() {
    let wrong = vec![7u8; SIZE];
    let rig = Rig::new(FakeNet::new(move |_, _, _| Ok(Some(whole(&wrong)))), u64::MAX);
    let dir = tempfile::tempdir().expect("dir");
    let ran = Arc::new(AtomicBool::new(false));
    rig.run("m", plan_of("https://x/m.bin", file_of("m.bin", &data(SIZE)), dir.path()), installer(&ran));
    let finished = rig.sink.finished().expect("announced");
    assert_eq!(finished.outcome, Outcome::Failed);
    assert!(finished.error.is_some_and(|e| e.contains("failed verification")));
    assert!(!dir.path().join("m.bin").exists() && !dir.path().join("m.bin.partial").exists());
    assert!(!ran.load(Ordering::SeqCst));
}

/// A failing installer fails the download after the files arrived.
#[test]
fn a_failure_while_installing_is_reported() {
    let bytes = data(SIZE);
    let body = bytes.clone();
    let rig = Rig::new(FakeNet::new(move |_, _, _| Ok(Some(whole(&body)))), u64::MAX);
    let dir = tempfile::tempdir().expect("dir");
    rig.run("m", plan_of("https://x/m.bin", file_of("m.bin", &bytes), dir.path()), Box::new(|_| Err("Could not unpack the model.".into())));
    let finished = rig.sink.finished().expect("announced");
    assert_eq!((finished.outcome, finished.error.as_deref()), (Outcome::Failed, Some("Could not unpack the model.")));
}

/// A partial file resumes with a request from its length, and only the rest crosses the network.
#[test]
fn a_partial_file_resumes_from_its_length() {
    let bytes = data(SIZE);
    let body = bytes.clone();
    let rig = Rig::new(FakeNet::new(move |_, have, _| Ok(Some(resumed(&body, have as usize)))), u64::MAX);
    let dir = tempfile::tempdir().expect("dir");
    fs::write(dir.path().join("m.bin.partial"), &bytes[..50_000]).expect("partial");
    let ran = Arc::new(AtomicBool::new(false));
    rig.run("m", plan_of("https://x/m.bin", file_of("m.bin", &bytes), dir.path()), installer(&ran));
    assert_eq!(rig.net.calls(), vec![("https://x/m.bin".to_string(), 50_000)]);
    assert_eq!(fs::read(dir.path().join("m.bin")).expect("file"), bytes);
    assert_eq!(rig.sink.finished().map(|f| f.outcome), Some(Outcome::Succeeded));
}

/// A drive without room for the re-planned download fails it before any byte is fetched.
#[test]
fn a_full_drive_fails_the_job_before_fetching() {
    let rig = Rig::new(FakeNet::new(|_, _, _| panic!("nothing may be fetched")), 10);
    let dir = tempfile::tempdir().expect("dir");
    let ran = Arc::new(AtomicBool::new(false));
    rig.run("m", plan_of("https://x/m.bin", file_of("m.bin", &data(SIZE)), dir.path()), installer(&ran));
    let finished = rig.sink.finished().expect("announced");
    assert_eq!((finished.outcome, finished.error.as_deref()), (Outcome::Failed, Some("Not enough free disk space for this download.")));
    assert!(rig.net.calls().is_empty());
}

/// A cancel while the connection is being made (the network blocks until its interrupt fires) ends the job as cancelled and installs nothing.
#[test]
fn cancel_during_connect_ends_the_job_as_cancelled() {
    let (entered, waiting) = channel();
    let rig = Rig::new(
        FakeNet::new(move |_, _, ctl| {
            hang(ctl, &entered);
            Err(Stop::Cancelled)
        }),
        u64::MAX,
    );
    let dir = tempfile::tempdir().expect("dir");
    let ran = Arc::new(AtomicBool::new(false));
    let (generation, _ctl, thread) = rig.spawn("m", plan_of("https://x/m.bin", file_of("m.bin", &data(SIZE)), dir.path()), installer(&ran));
    await_entered(&waiting);
    rig.env.manager.cancel("m", generation);
    thread.join().expect("job");
    assert_eq!(rig.sink.finished().map(|f| f.outcome), Some(Outcome::Cancelled));
    assert!(!ran.load(Ordering::SeqCst) && !dir.path().join("m.bin").exists());
    assert!(rig.env.manager.view().job("m").is_none());
}

/// A cancel while the body read is stalled mid-file ends the job as cancelled and leaves no final file.
#[test]
fn cancel_during_read_ends_the_job_as_cancelled() {
    let (entered, waiting) = channel();
    let bytes = data(SIZE);
    let body = bytes[..1000].to_vec();
    let rig = Rig::new(
        FakeNet::new(move |_, _, ctl| {
            let stalled = StallingBody::new(body.clone(), ctl, entered.clone());
            Ok(Some(Fetched { status: 200, content_range: None, body: Box::new(stalled) }))
        }),
        u64::MAX,
    );
    let dir = tempfile::tempdir().expect("dir");
    let ran = Arc::new(AtomicBool::new(false));
    let (generation, _ctl, thread) = rig.spawn("m", plan_of("https://x/m.bin", file_of("m.bin", &bytes), dir.path()), installer(&ran));
    await_entered(&waiting);
    rig.env.manager.cancel("m", generation);
    thread.join().expect("job");
    assert_eq!(rig.sink.finished().map(|f| f.outcome), Some(Outcome::Cancelled));
    assert!(!ran.load(Ordering::SeqCst) && !dir.path().join("m.bin").exists());
}

/// A cancel that is already set when the job reaches verification stops it before any request.
#[test]
fn cancel_before_verification_stops_without_a_request() {
    let rig = Rig::new(FakeNet::new(|_, _, _| panic!("nothing may be fetched")), u64::MAX);
    let bytes = data(SIZE);
    let dir = tempfile::tempdir().expect("dir");
    fs::write(dir.path().join("m.bin.partial"), &bytes[..100_000]).expect("partial");
    let plan = plan_of("https://x/m.bin", file_of("m.bin", &bytes), dir.path());
    let (generation, ctl) = rig.admit("m", &plan);
    rig.env.manager.cancel("m", generation);
    crate::model_download::job::run(&rig.env, "m", generation, &ctl, &plan, Box::new(|_| Ok(())));
    assert_eq!(rig.sink.finished().map(|f| f.outcome), Some(Outcome::Cancelled));
    assert!(rig.net.calls().is_empty());
}

/// A cancel while the installer works (it blocks until its interrupt fires, as a killed unpacker does) ends the job as cancelled and the installer leaves nothing behind.
#[test]
fn cancel_during_install_ends_the_job_as_cancelled() {
    let bytes = data(SIZE);
    let body = bytes.clone();
    let rig = Rig::new(FakeNet::new(move |_, _, _| Ok(Some(whole(&body)))), u64::MAX);
    let dir = tempfile::tempdir().expect("dir");
    let (entered, waiting) = channel();
    let (installed, marker) = (Arc::new(AtomicBool::new(false)), dir.path().join("installed.marker"));
    let (flag, written) = (installed.clone(), marker.clone());
    let after: After = Box::new(move |ctl| {
        hang(ctl, &entered);
        if ctl.is_cancelled() {
            return Err("killed".into());
        }
        flag.store(true, Ordering::SeqCst);
        fs::write(written, b"done").map_err(|e| e.to_string())
    });
    let (generation, _ctl, thread) = rig.spawn("m", plan_of("https://x/m.bin", file_of("m.bin", &bytes), dir.path()), after);
    await_entered(&waiting);
    rig.env.manager.cancel("m", generation);
    thread.join().expect("job");
    let finished = rig.sink.finished().expect("announced");
    assert_eq!((finished.outcome, finished.error), (Outcome::Cancelled, None));
    assert!(!installed.load(Ordering::SeqCst) && !marker.exists());
    assert_eq!(rig.env.manager.view().last("m").map(|l| l.outcome), Some(Outcome::Cancelled));
}
