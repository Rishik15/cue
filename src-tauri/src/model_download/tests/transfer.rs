//! File: model_download/tests/transfer.rs
//! Purpose: Protect fetching one model file from a real (loopback) server: verified before it gets its name, resumable, cancellable, and safe against a server that lies about ranges, redirects or content.
//! Contents: scenario tests of `fetch_retrying` with the server in `server`.

use std::fs;
use std::path::Path;

use super::server::{data, file_of, Reply, Server};
use crate::model_download::http::HttpNet;
use crate::model_download::state::{Manager, Sizes};
use crate::model_download::transfer::{fetch_retrying, Stop};

const SIZE: usize = 300_000;

/// A fresh table of downloads with one admitted download "model", as the app starts one. Returns the table and the download's generation.
fn admitted() -> (&'static Manager, u64, std::sync::Arc<crate::model_download::state::Ctl>) {
    let manager: &'static Manager = Box::leak(Box::new(Manager::new()));
    let job = manager.admit("model", "speech", Sizes { total: SIZE as u64, claims: vec![] }, &|| u64::MAX).expect("admitted");
    (manager, job.generation, job.ctl)
}

/// Fetches `name` into `dir` from `url`, collecting the progress reports. Returns the outcome and the last reported byte count.
fn fetch(url: &str, bytes: &[u8], dir: &Path) -> (Result<(), Stop>, u64) {
    let (_, _, ctl) = admitted();
    let (mut last, file) = (0, file_of("model.bin", bytes));
    let outcome = fetch_retrying(&HttpNet, url, file, dir, &ctl, &mut |done: u64| last = done);
    (outcome, last)
}

/// A whole download arrives, matches its hash and only then gets its real name; no partial file is left.
#[test]
fn a_download_is_verified_and_renamed_only_when_complete() {
    let bytes = data(SIZE);
    let body = bytes.clone();
    let server = Server::start(move |_| Reply::ok(&body));
    let dir = tempfile::tempdir().expect("dir");
    let (outcome, last) = fetch(&format!("{}/model.bin", server.url), &bytes, dir.path());
    assert!(outcome.is_ok(), "{outcome:?}");
    assert_eq!(last, SIZE as u64);
    assert_eq!(fs::read(dir.path().join("model.bin")).expect("final file"), bytes);
    assert!(!dir.path().join("model.bin.partial").exists());
}

/// A partial file from an earlier run resumes with a Range request from the right byte, and the finished file is byte-for-byte right.
#[test]
fn a_partial_download_resumes_from_where_it_stopped() {
    let bytes = data(SIZE);
    let body = bytes.clone();
    let server = Server::start(move |request| match &request.range {
        Some(range) if range == "bytes=100000-" => Reply::partial(&body[100_000..], 100_000, body.len()),
        _ => Reply::ok(&body),
    });
    let dir = tempfile::tempdir().expect("dir");
    fs::write(dir.path().join("model.bin.partial"), &bytes[..100_000]).expect("partial");
    let (outcome, _) = fetch(&format!("{}/model.bin", server.url), &bytes, dir.path());
    assert!(outcome.is_ok(), "{outcome:?}");
    assert_eq!(fs::read(dir.path().join("model.bin")).expect("final file"), bytes);
    assert_eq!(server.requests().len(), 1);
    assert_eq!(server.requests()[0].range.as_deref(), Some("bytes=100000-"));
}

/// A dropped connection keeps what arrived and the retry resumes from there instead of starting over.
#[test]
fn a_dropped_connection_is_retried_and_resumed() {
    let bytes = data(SIZE);
    let body = bytes.clone();
    let server = Server::start(move |request| match &request.range {
        None => Reply::ok(&body).cut_at(150_000),
        Some(range) => {
            let from: usize = range.trim_start_matches("bytes=").trim_end_matches('-').parse().expect("range start");
            Reply::partial(&body[from..], from, body.len())
        }
    });
    let dir = tempfile::tempdir().expect("dir");
    let (outcome, _) = fetch(&format!("{}/model.bin", server.url), &bytes, dir.path());
    assert!(outcome.is_ok(), "{outcome:?}");
    assert_eq!(fs::read(dir.path().join("model.bin")).expect("final file"), bytes);
    let requests = server.requests();
    assert_eq!(requests.len(), 2);
    assert!(requests[1].range.as_deref().is_some_and(|r| r.starts_with("bytes=") && r != "bytes=0-"));
}

/// A server that ignores the Range request and sends the whole file does not corrupt the download: the old partial is discarded.
#[test]
fn a_server_that_ignores_the_range_still_gives_a_correct_file() {
    let bytes = data(SIZE);
    let body = bytes.clone();
    let server = Server::start(move |_| Reply::ok(&body));
    let dir = tempfile::tempdir().expect("dir");
    fs::write(dir.path().join("model.bin.partial"), &bytes[..40_000]).expect("partial");
    let (outcome, _) = fetch(&format!("{}/model.bin", server.url), &bytes, dir.path());
    assert!(outcome.is_ok(), "{outcome:?}");
    assert_eq!(fs::read(dir.path().join("model.bin")).expect("final file"), bytes);
}

/// A 416 answer (the partial does not fit the file on the server) restarts once from the beginning.
#[test]
fn a_rejected_range_restarts_from_zero() {
    let bytes = data(SIZE);
    let body = bytes.clone();
    let server = Server::start(move |request| if request.range.is_some() { Reply::with(416, b"") } else { Reply::ok(&body) });
    let dir = tempfile::tempdir().expect("dir");
    fs::write(dir.path().join("model.bin.partial"), b"garbage that is not a prefix").expect("partial");
    let (outcome, _) = fetch(&format!("{}/model.bin", server.url), &bytes, dir.path());
    assert!(outcome.is_ok(), "{outcome:?}");
    assert_eq!(fs::read(dir.path().join("model.bin")).expect("final file"), bytes);
}

/// A resume that starts at the wrong place is refused and the partial is removed, so wrong bytes never end up inside the file.
#[test]
fn a_resume_at_the_wrong_offset_fails_and_drops_the_partial() {
    let bytes = data(SIZE);
    let body = bytes.clone();
    let server = Server::start(move |_| Reply::partial(&body[50_000..], 50_000, body.len()));
    let dir = tempfile::tempdir().expect("dir");
    fs::write(dir.path().join("model.bin.partial"), &bytes[..100_000]).expect("partial");
    let (outcome, _) = fetch(&format!("{}/model.bin", server.url), &bytes, dir.path());
    assert!(matches!(&outcome, Err(Stop::Failed(m)) if m.contains("wrong place")), "{outcome:?}");
    assert!(!dir.path().join("model.bin.partial").exists());
    assert!(!dir.path().join("model.bin").exists());
}

/// Bytes that do not match the pinned hash never get the real name, and the partial is thrown away so a retry starts clean.
#[test]
fn a_corrupted_download_is_rejected_and_never_named() {
    let bytes = data(SIZE);
    let mut tampered = bytes.clone();
    tampered[1234] ^= 0xFF;
    let server = Server::start(move |_| Reply::ok(&tampered));
    let dir = tempfile::tempdir().expect("dir");
    let (outcome, _) = fetch(&format!("{}/model.bin", server.url), &bytes, dir.path());
    assert!(matches!(&outcome, Err(Stop::Failed(m)) if m.contains("failed verification")), "{outcome:?}");
    assert!(!dir.path().join("model.bin").exists());
    assert!(!dir.path().join("model.bin.partial").exists());
}

/// A finished, genuine file is reused without any request; a same-size file with the wrong content is replaced.
#[test]
fn a_genuine_existing_file_is_reused_and_a_damaged_one_is_replaced() {
    let bytes = data(SIZE);
    let body = bytes.clone();
    let server = Server::start(move |_| Reply::ok(&body));
    let dir = tempfile::tempdir().expect("dir");
    let url = format!("{}/model.bin", server.url);
    fs::write(dir.path().join("model.bin"), &bytes).expect("genuine");
    let (outcome, last) = fetch(&url, &bytes, dir.path());
    assert!(outcome.is_ok() && last == SIZE as u64);
    assert!(server.requests().is_empty(), "a genuine file must not be fetched again");
    fs::write(dir.path().join("model.bin"), vec![7u8; SIZE]).expect("damaged");
    let (outcome, _) = fetch(&url, &bytes, dir.path());
    assert!(outcome.is_ok(), "{outcome:?}");
    assert_eq!(fs::read(dir.path().join("model.bin")).expect("replaced"), bytes);
}

/// Cancelling mid-way ends the fetch as cancelled, keeps the partial for a later resume and never creates the final file.
#[test]
fn cancel_mid_download_keeps_the_partial_and_never_names_the_file() {
    let bytes = data(SIZE);
    let body = bytes.clone();
    let server = Server::start(move |_| Reply::ok(&body));
    let dir = tempfile::tempdir().expect("dir");
    let (manager, generation, ctl) = admitted();
    let outcome =
        fetch_retrying(&HttpNet, &format!("{}/model.bin", server.url), file_of("model.bin", &bytes), dir.path(), &ctl, &mut |_: u64| {
            manager.cancel("model", generation);
        });
    assert!(matches!(outcome, Err(Stop::Cancelled)), "{outcome:?}");
    assert!(!dir.path().join("model.bin").exists());
    assert!(dir.path().join("model.bin.partial").exists());
}

/// The server's errors map to the right ending: a missing file fails with a plain message, and a model is never downloaded over plain HTTP from anywhere but the test loopback.
#[test]
fn errors_and_insecure_addresses_fail_with_plain_messages() {
    let bytes = data(1000);
    let server = Server::start(|_| Reply::with(404, b"no"));
    let dir = tempfile::tempdir().expect("dir");
    let (missing, _) = fetch(&format!("{}/model.bin", server.url), &bytes, dir.path());
    assert!(matches!(&missing, Err(Stop::Failed(m)) if m.contains("not available")), "{missing:?}");
    let (insecure, _) = fetch("http://example.com/model.bin", &bytes, dir.path());
    assert!(matches!(&insecure, Err(Stop::Failed(m)) if m.contains("HTTPS")), "{insecure:?}");
}

/// Each case gets its own folder, so a finished file from one cannot answer the next. Redirects are followed by hand: a relative redirect works, a loop is cut off, and a redirect to plain HTTP elsewhere is refused.
#[test]
fn redirects_are_followed_safely() {
    let bytes = data(5000);
    let body = bytes.clone();
    let server = Server::start(move |request| match request.path.as_str() {
        "/old" => Reply::redirect("/new/path"),
        "/new/path" => Reply::ok(&body),
        "/loop" => Reply::redirect("/loop"),
        _ => Reply::redirect("http://example.com/elsewhere"),
    });
    let (followed, _) = fetch(&format!("{}/old", server.url), &bytes, tempfile::tempdir().expect("dir").path());
    assert!(followed.is_ok(), "{followed:?}");
    let (looping, _) = fetch(&format!("{}/loop", server.url), &bytes, tempfile::tempdir().expect("dir").path());
    assert!(matches!(&looping, Err(Stop::Failed(m)) if m.contains("redirected too many times")), "{looping:?}");
    let (downgraded, _) = fetch(&format!("{}/away", server.url), &bytes, tempfile::tempdir().expect("dir").path());
    assert!(matches!(&downgraded, Err(Stop::Failed(m)) if m.contains("HTTPS")), "{downgraded:?}");
}
