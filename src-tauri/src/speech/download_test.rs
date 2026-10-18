//! Purpose: Prove downloads resume after a dropped connection, through a redirect, and survive a server that ignores Range.
//! Contents: serve — a loopback HTTP server (redirect plus a file, optionally cut short once); the tests drive `fetch` against it.

use std::io::{BufRead, BufReader, Write};
use std::net::{Shutdown, TcpListener};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use super::*;

type Requests = Arc<Mutex<Vec<(String, Option<String>)>>>;

fn body() -> Vec<u8> {
    (0..200_000u32).map(|i| (i % 251) as u8).collect()
}

fn model_file(name: &'static str, data: &[u8]) -> ModelFile {
    let sha256: &'static str = Box::leak(format!("{:x}", Sha256::digest(data)).into_boxed_str());
    ModelFile { name, size: data.len() as u64, sha256 }
}

/// `/redirect` answers 302 to `/file`. The first `/file` answer is cut at half when `cut_first`; `honour_range` decides 206 or 200.
fn serve(cut_first: bool, honour_range: bool) -> (String, Requests) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/redirect", listener.local_addr().unwrap());
    let seen: Requests = Arc::default();
    let log = seen.clone();
    let cut = AtomicBool::new(cut_first);
    let data = body();
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let mut lines = BufReader::new(stream.try_clone().unwrap()).lines().map_while(Result::ok);
            let path = lines.next().unwrap_or_default().split(' ').nth(1).unwrap_or("").to_string();
            let range = lines.take_while(|l| !l.is_empty()).find_map(|l| l.strip_prefix("Range: ").map(str::to_string));
            log.lock().unwrap().push((path.clone(), range.clone()));
            if path == "/redirect" {
                let _ = stream.write_all(b"HTTP/1.1 302 Found\r\nLocation: /file\r\nContent-Length: 0\r\n\r\n");
                continue;
            }
            let start = range.as_deref().filter(|_| honour_range).and_then(|r| r.strip_prefix("bytes=")?.strip_suffix('-')?.parse().ok());
            let tail = &data[start.unwrap_or(0)..];
            let (status, range_header) = match start {
                Some(from) => ("206 Partial Content", format!("Content-Range: bytes {from}-{}/{}\r\n", data.len() - 1, data.len())),
                None => ("200 OK", String::new()),
            };
            let _ = write!(stream, "HTTP/1.1 {status}\r\n{range_header}Content-Length: {}\r\n\r\n", tail.len());
            let sent = if cut.swap(false, Ordering::SeqCst) { tail.len() / 2 } else { tail.len() };
            let _ = stream.write_all(&tail[..sent]);
            let _ = stream.shutdown(Shutdown::Both);
        }
    });
    (url, seen)
}

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("cue-download-test-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn run_fetch(url: &str, file: &ModelFile, dir: &Path) -> Result<(), Stop> {
    fetch(url, file, dir, &AtomicBool::new(false), &mut |_| {})
}

#[test]
fn resumes_after_a_dropped_connection_through_a_redirect() {
    let data = body();
    let file = model_file("a.bin", &data);
    let (url, seen) = serve(true, true);
    let dir = temp_dir("resume");

    assert!(run_fetch(&url, &file, &dir).is_err());
    let partial = fs::metadata(dir.join("a.bin.partial")).expect("partial kept").len();
    assert!(partial > 0 && partial < file.size);

    run_fetch(&url, &file, &dir).unwrap();
    assert_eq!(fs::read(dir.join("a.bin")).unwrap(), data);
    assert!(!dir.join("a.bin.partial").exists());
    let resumed = seen.lock().unwrap().iter().filter(|(p, _)| p == "/file").filter_map(|(_, r)| r.clone()).next_back();
    assert_eq!(resumed, Some(format!("bytes={partial}-")));
}

#[test]
fn restarts_when_the_server_ignores_range() {
    let data = body();
    let file = model_file("b.bin", &data);
    let (url, _) = serve(true, false);
    let dir = temp_dir("norange");

    assert!(run_fetch(&url, &file, &dir).is_err());
    run_fetch(&url, &file, &dir).unwrap();
    assert_eq!(fs::read(dir.join("b.bin")).unwrap(), data);
}

#[test]
fn a_wrong_hash_discards_the_file() {
    let data = body();
    let file = ModelFile { sha256: "00", ..model_file("c.bin", &data) };
    let (url, _) = serve(false, true);
    let dir = temp_dir("badhash");

    assert!(run_fetch(&url, &file, &dir).is_err());
    assert!(!dir.join("c.bin").exists() && !dir.join("c.bin.partial").exists());
}

#[test]
fn plain_http_is_refused() {
    let data = body();
    let dir = std::env::temp_dir().join("cue-download-http-test");
    let _ = fs::create_dir_all(&dir);
    let outcome = run_fetch("http://example.com/model.bin", &model_file("h.bin", &data), &dir);
    assert!(matches!(outcome, Err(Stop::Failed(ref m)) if m.contains("HTTPS")), "{outcome:?}");
    assert!(!dir.join("h.bin").exists());
    let _ = fs::remove_dir_all(&dir);
}
