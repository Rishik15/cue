//! File: model_download/tests/edges.rs
//! Purpose: Protect the edge cases of fetching: a server that is busy and names no wait, a wait that a cancel ends at once, a body longer than the file, a redirect whose address has a slash in its query, and the text of a failed request.
//! Contents: scenario tests of `fetch_retrying`, `next_url` and `call_failure`.

use std::fs;
use std::time::{Duration, Instant};

use super::server::{data, file_of, Reply, Server};
use crate::model_download::ctl::Ctl;
use crate::model_download::http::{call_failure, next_url, HttpNet};
use crate::model_download::state::{Manager, Sizes};
use crate::model_download::transfer::{fetch_retrying, Stop};

/// A 503 that asks for no wait at all and a partial that never grows: the download retries without delay, then gives up with the busy message after the last retry.
#[test]
fn a_busy_server_with_no_wait_and_no_progress_gives_up() {
    let bytes = data(1000);
    let server = Server::start(|_| {
        let mut reply = Reply::with(503, b"");
        reply.headers.push(("Retry-After", "0".into()));
        reply
    });
    let dir = tempfile::tempdir().expect("dir");
    let ctl = Ctl::detached();
    let outcome = fetch_retrying(&HttpNet, &format!("{}/m.bin", server.url), file_of("m.bin", &bytes), dir.path(), &ctl, &mut |_: u64| {});
    assert!(matches!(&outcome, Err(Stop::Failed(m)) if m.contains("busy")), "{outcome:?}");
    assert_eq!(server.requests().len(), 7, "one try and six retries");
}

/// A cancel during a long Retry-After wait ends it at once instead of after the wait.
#[test]
fn a_cancel_ends_a_retry_wait_at_once() {
    let bytes = data(1000);
    let server = Server::start(|_| {
        let mut reply = Reply::with(503, b"");
        reply.headers.push(("Retry-After", "100".into()));
        reply
    });
    let dir = tempfile::tempdir().expect("dir");
    let manager: &'static Manager = Box::leak(Box::new(Manager::new()));
    let job = manager.admit("model", "speech", Sizes { total: 1000, claims: vec![] }, &|| u64::MAX).expect("admitted");
    let generation = job.generation;
    let canceller = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(300));
        manager.cancel("model", generation);
    });
    let started = Instant::now();
    let outcome =
        fetch_retrying(&HttpNet, &format!("{}/m.bin", server.url), file_of("m.bin", &bytes), dir.path(), &job.ctl, &mut |_: u64| {});
    canceller.join().expect("canceller");
    assert!(matches!(outcome, Err(Stop::Cancelled)), "{outcome:?}");
    assert!(started.elapsed() < Duration::from_secs(5), "waited {:?}", started.elapsed());
}

/// A resume whose body runs past the end of the file still ends with the right file: the extra bytes are never written.
#[test]
fn a_body_longer_than_the_file_is_cut_at_its_size() {
    let bytes = data(300_000);
    let mut longer = bytes[100_000..].to_vec();
    longer.extend_from_slice(&[9u8; 5000]);
    let server = Server::start(move |_| Reply::partial(&longer, 100_000, 300_000));
    let dir = tempfile::tempdir().expect("dir");
    fs::write(dir.path().join("m.bin.partial"), &bytes[..100_000]).expect("partial");
    let ctl = Ctl::detached();
    let outcome = fetch_retrying(&HttpNet, &format!("{}/m.bin", server.url), file_of("m.bin", &bytes), dir.path(), &ctl, &mut |_: u64| {});
    assert!(outcome.is_ok(), "{outcome:?}");
    assert_eq!(fs::read(dir.path().join("m.bin")).expect("final file"), bytes);
}

/// A relative redirect resolves against the path of the current address, even when its query holds a slash.
#[test]
fn a_slash_in_the_query_does_not_move_a_relative_redirect() {
    let next = next_url("https://host.example/models/a/file?token=x/y", Some("other.bin")).expect("url");
    assert_eq!(next, "https://host.example/models/a/other.bin");
    let root = next_url("https://host.example?token=x/y", Some("/other.bin")).expect("url");
    assert_eq!(root, "https://host.example/other.bin");
}

/// A certificate or address problem fails with its own text and is not retried; a plain connection problem is retried; a cancel wins over both.
#[test]
fn a_failed_request_is_told_apart_by_its_cause() {
    let ctl = Ctl::detached();
    assert!(matches!(call_failure(&ureq::Error::Tls("bad"), &ctl), Stop::Failed(m) if m.contains("secure connection")));
    assert!(matches!(call_failure(&ureq::Error::BadUri("x".into()), &ctl), Stop::Failed(m) if m.contains("not valid")));
    assert!(matches!(call_failure(&ureq::Error::HostNotFound, &ctl), Stop::Offline(_)));
    ctl.cancel_for_test();
    assert!(matches!(call_failure(&ureq::Error::Tls("bad"), &ctl), Stop::Cancelled));
}
