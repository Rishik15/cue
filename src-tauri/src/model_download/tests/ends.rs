//! File: model_download/tests/ends.rs
//! Purpose: Protect that a download always ends: a panic on a network helper is a failed call, not a wait, and a thread that cannot start still announces the end of the job.
//! Contents: interruptible and PumpedBody with a panicking helper; launch with a failing spawner.

use std::io::{self, Read};
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::time::Duration;

use super::fakes::{plan_of, Event, FakeDisk, FakeNet, Recorder, Rig};
use super::server::file_of;
use crate::model_download::ctl::Ctl;
use crate::model_download::interrupt::{interruptible, PumpedBody};
use crate::model_download::job::Env;
use crate::model_download::net::Fetched;
use crate::model_download::state::{Manager, Outcome};
use crate::model_download::transfer::Stop;
use crate::model_download::{launch, After, Plan};

/// Runs `check` on a thread and fails the test when it does not return within 2 s.
fn within_2s<T: Send + 'static>(check: impl FnOnce() -> T + Send + 'static) -> T {
    let (tx, rx) = channel();
    std::thread::spawn(move || drop(tx.send(check())));
    rx.recv_timeout(Duration::from_secs(2)).expect("the call hung")
}

/// A body whose read panics.
struct Boom;

impl Read for Boom {
    /// Panics; input is the unused buffer, there is no return.
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        panic!("boom")
    }
}

/// A helper that panics is reported as a failed call and leaves no interrupt behind.
#[test]
fn interruptible_reports_a_panicking_helper() {
    let ctl = Ctl::detached();
    let own = ctl.clone();
    let result = within_2s(move || interruptible(&own, || -> u8 { panic!("boom") }));
    assert!(matches!(result, Err(Stop::Failed(_))));
    assert_eq!(ctl.interrupt_count(), 0);
}

/// A read that panics is an `io::Error`, not a cancel and not a wait.
#[test]
fn pumped_body_reports_a_panicking_read() {
    let ctl = Ctl::detached();
    let err = within_2s(move || {
        let mut body = PumpedBody::new(Boom, &ctl, vec![]);
        body.read_to_end(&mut Vec::new()).unwrap_err()
    });
    assert_ne!(err.kind(), io::ErrorKind::Interrupted);
}

/// A download whose thread cannot start is refused, and the last event carries the revision without the job.
#[test]
fn a_failed_spawn_announces_the_end() {
    let manager: &'static Manager = Box::leak(Box::new(Manager::new()));
    let sink = Arc::new(Recorder::default());
    let env = Env {
        manager,
        net: Arc::new(FakeNet::new(|_, _, _| Err(Stop::Cancelled))),
        disk: Arc::new(FakeDisk { free: u64::MAX }),
        sink: sink.clone(),
    };
    let plan: Plan = Box::new(|| Ok(vec![]));
    let after: After = Box::new(|_| Ok(()));
    let result = launch(env, "m", "speech", plan, after, |_| Err(io::Error::other("no threads")));
    assert!(result.is_err());
    let view = manager.view();
    assert!(view.job("m").is_none());
    assert_eq!(sink.events().last(), Some(&Event::Changed(view.revision)));
}

/// A body helper that panics ends the whole job as Failed in bounded time: the job leaves the table and the network is asked once, so nothing retries.
#[test]
fn a_panicking_body_helper_fails_the_job_without_a_retry() {
    let dir = tempfile::tempdir().expect("dir");
    let file = file_of("m.bin", b"data");
    let rig = Arc::new(Rig::new(
        FakeNet::new(|_, _, ctl| {
            let body = PumpedBody::new(Boom, ctl, vec![]);
            Ok(Some(Fetched { status: 200, content_range: None, etag: None, encoding: None, body: Box::new(body) }))
        }),
        u64::MAX,
    ));
    let (run, plan) = (rig.clone(), plan_of("https://x/m.bin", file, dir.path()));
    within_2s(move || run.run("m", plan, Box::new(|_| Ok(()))));
    let finished = rig.sink.finished().expect("announced");
    assert_eq!(finished.outcome, Outcome::Failed);
    assert!(rig.env.manager.view().job("m").is_none());
    assert_eq!(rig.net.calls().len(), 1);
}
