//! File: model_download/tests/silent.rs
//! Purpose: Protect that Cancel ends a download talking to a silent server within a second: one that accepts and never answers, and one that stalls in the middle of the body.
//! Contents: loopback scenarios of `HttpNet` with a cancel from another thread.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::{Duration, Instant};

use crate::model_download::http::HttpNet;
use crate::model_download::net::Net;
use crate::model_download::state::{Manager, Sizes};
use crate::model_download::transfer::Stop;

/// The bound the user feels: a cancel is over within this.
const BOUND: Duration = Duration::from_secs(1);

/// A table with one admitted download "m"; returns the table, its generation and its cancel object.
fn admitted() -> (&'static Manager, u64, std::sync::Arc<crate::model_download::state::Ctl>) {
    let manager: &'static Manager = Box::leak(Box::new(Manager::new()));
    let job = manager.admit("m", "speech", Sizes { total: 1, claims: vec![] }, &|| u64::MAX).expect("admitted");
    (manager, job.generation, job.ctl)
}

/// Cancels "m" 200 ms from now on another thread.
fn cancel_soon(manager: &'static Manager, generation: u64) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(200));
        manager.cancel("m", generation);
    });
}

/// A server that accepts the connection and never answers: opening ends as cancelled within the bound, long before the 30 s response deadline.
#[test]
fn cancel_ends_a_request_the_server_never_answers() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let url = format!("http://127.0.0.1:{}/m.bin", listener.local_addr().expect("addr").port());
    let _holder = std::thread::spawn(move || {
        let held = listener.accept();
        std::thread::sleep(Duration::from_secs(5));
        drop(held);
    });
    let (manager, generation, ctl) = admitted();
    cancel_soon(manager, generation);
    let started = Instant::now();
    let outcome = HttpNet.open(&url, 0, &ctl);
    assert!(matches!(outcome, Err(Stop::Cancelled)), "{:?}", outcome.map(|o| o.is_some()));
    assert!(started.elapsed() < BOUND, "took {:?}", started.elapsed());
}

/// A server that sends the headers and some body, then goes silent: the read ends within the bound of the cancel.
#[test]
fn cancel_ends_a_body_read_that_went_silent() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let url = format!("http://127.0.0.1:{}/m.bin", listener.local_addr().expect("addr").port());
    let _server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let mut request = [0u8; 1024];
        let _ = stream.read(&mut request);
        stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1000000\r\n\r\nabc").expect("write");
        std::thread::sleep(Duration::from_secs(5));
    });
    let (manager, generation, ctl) = admitted();
    let mut body = HttpNet.open(&url, 0, &ctl).expect("opened").expect("answer").body;
    let mut first = [0u8; 3];
    body.read_exact(&mut first).expect("first bytes");
    cancel_soon(manager, generation);
    let started = Instant::now();
    let mut more = [0u8; 16];
    assert!(body.read(&mut more).is_err());
    assert!(started.elapsed() < BOUND, "took {:?}", started.elapsed());
}
