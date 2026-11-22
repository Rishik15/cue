//! File: text/tests/backend.rs
//! Purpose: Protect the HTTP side of the text model: how a reply's `finish_reason` becomes "cut", a failure to reach the server, and that the cancel watchdog frees a request blocked on the server.
//! Contents: a stand-in HTTP server on a local port; tests of `request` (length, stop, the one-token warm-up, an unreachable server) and of `watch` (a cancel runs the abort, no cancel never does, a cancel unblocks a waiting request).

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, RecvTimeoutError};
use std::sync::Arc;
use std::time::Duration;

use crate::actions::{task, Action};
use crate::text::backend::{request, watch, Reply};
use crate::text::cancel::Cancel;
use crate::text::engine::Conn;

/// Reads one HTTP request (headers, then the body the Content-Length names) from a client. Input: the buffered stream.
fn read_request(reader: &mut BufReader<std::net::TcpStream>) {
    let mut length = 0;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
            break;
        }
        if let Some(value) = line.to_lowercase().strip_prefix("content-length:") {
            length = value.trim().parse().unwrap_or(0);
        }
    }
    let mut body = vec![0; length];
    let _ = reader.read_exact(&mut body);
}

/// A server on a free local port that answers its first request with `body` as JSON. Input: the body. Returns how to reach it.
fn serve(body: &'static str) -> Conn {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        read_request(&mut reader);
        let head =
            format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
        let _ = (&stream).write_all(format!("{head}{body}").as_bytes());
    });
    Conn { port, key: "k".into() }
}

/// What `request` returns for a reply of `body` when asked for `max_tokens`.
fn asked(body: &'static str, max_tokens: usize) -> Result<Reply, String> {
    request(&serve(body), task(Action::Correct).expect("model task"), "text", max_tokens)
}

/// A reply that stopped on its own is not cut; one that hit the limit is; the one-token warm-up always hits it and is never marked.
#[test]
fn finish_reason_length_marks_a_cut_reply_except_for_the_warm_up() {
    let stopped = r#"{"choices":[{"finish_reason":"stop","message":{"content":"Done."}}]}"#;
    let length = r#"{"choices":[{"finish_reason":"length","message":{"content":"Do"}}]}"#;
    assert_eq!(asked(stopped, 50), Ok(Reply { text: "Done.".into(), cut: false }));
    assert_eq!(asked(length, 50), Ok(Reply { text: "Do".into(), cut: true }));
    assert_eq!(asked(length, 1), Ok(Reply { text: "Do".into(), cut: false }));
}

/// A server that is not there is a readable failure, not a panic.
#[test]
fn an_unreachable_server_is_a_failure_with_a_message() {
    let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let failed = request(&Conn { port, key: "k".into() }, task(Action::Correct).expect("model task"), "text", 10);
    assert!(failed.unwrap_err().starts_with("The text engine did not answer"));
}

/// The watchdog runs its abort once the token is set, and never when it is retired first.
#[test]
fn the_watchdog_aborts_on_cancel_and_not_after_it_is_retired() {
    let (aborted, seen) = channel::<()>();
    let flag = Arc::new(AtomicBool::new(false));
    let cancel = {
        let flag = Arc::clone(&flag);
        Cancel::new(move || flag.load(Ordering::SeqCst))
    };
    let _watchdog = watch(move || aborted.send(()).unwrap(), cancel.clone()).unwrap();
    assert_eq!(seen.recv_timeout(Duration::from_millis(300)), Err(RecvTimeoutError::Timeout));
    flag.store(true, Ordering::SeqCst);
    assert_eq!(seen.recv_timeout(Duration::from_secs(5)), Ok(()));

    let (aborted, seen) = channel::<()>();
    drop(watch(move || aborted.send(()).unwrap(), Cancel::never()));
    assert_eq!(seen.recv_timeout(Duration::from_secs(5)), Err(RecvTimeoutError::Disconnected));
}

/// A request blocked on a server that never answers fails as soon as the watchdog's abort closes the connection after the cancel.
#[test]
fn a_cancel_reaches_a_request_blocked_on_the_server() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let conn = Conn { port: listener.local_addr().unwrap().port(), key: "k".into() };
    let (release, released) = channel::<()>();
    std::thread::spawn(move || {
        let (_stream, _) = listener.accept().unwrap();
        let _ = released.recv_timeout(Duration::from_secs(30));
    });
    let flag = Arc::new(AtomicBool::new(false));
    let cancel = {
        let flag = Arc::clone(&flag);
        Cancel::new(move || flag.load(Ordering::SeqCst))
    };
    let _watchdog = watch(move || drop(release), cancel).unwrap();
    let blocked = std::thread::spawn(move || request(&conn, task(Action::Correct).expect("model task"), "text", 10));
    flag.store(true, Ordering::SeqCst);
    assert!(blocked.join().unwrap().is_err());
}
