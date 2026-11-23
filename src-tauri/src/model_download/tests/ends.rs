//! File: model_download/tests/ends.rs
//! Purpose: Protect that a download always ends: a panic on a network helper is a failed call, not a wait, and a thread that cannot start still announces the end of the job.
//! Contents: interruptible and PumpedBody with a panicking helper; launch with a failing spawner.

use std::io::{self, Read};
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::time::Duration;

use super::fakes::{Event, FakeDisk, FakeNet, Recorder};
use crate::model_download::ctl::Ctl;
use crate::model_download::interrupt::{interruptible, PumpedBody};
use crate::model_download::job::Env;
use crate::model_download::state::Manager;
use crate::model_download::transfer::Stop;
use crate::model_download::{launch, After, Plan};

/// Runs `check` on a thread and fails the test when it does not return within 2 s.
fn within_2s<T: Send + 'static>(check: impl FnOnce() -> T + Send + 'static) -> T {
    let (tx, rx) = channel();
    std::thread::spawn(move || drop(tx.send(check())));
    rx.recv_timeout(Duration::from_secs(2)).expect("the call hung")
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
    struct Boom;
    impl Read for Boom {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            panic!("boom")
        }
    }
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
