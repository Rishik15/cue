//! File: model_download/tests/server.rs
//! Purpose: A tiny HTTP server on the loopback address for download tests: it answers each request from a handler and records what was asked, so tests exercise real sockets with no mocking.
//! Contents: Request / Reply — what came in and what goes out; Server — start, requests, stop on drop; data — deterministic bytes; file_of — a model file entry for some bytes.

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use sha2::{Digest, Sha256};

use crate::speech::catalog::ModelFile;

/// One request as the server saw it.
#[derive(Clone, Debug)]
pub struct Request {
    pub path: String,
    pub range: Option<String>,
    pub if_range: Option<String>,
}

/// One response: status, extra headers and a body; `announce` is the Content-Length to claim when the body is deliberately cut short.
pub struct Reply {
    pub status: u16,
    pub headers: Vec<(&'static str, String)>,
    pub body: Vec<u8>,
    pub announce: Option<usize>,
}

impl Reply {
    /// A plain answer with a status and a body. Inputs: status code and body. Returns the reply.
    pub fn with(status: u16, body: &[u8]) -> Reply {
        Reply { status, headers: Vec::new(), body: body.to_vec(), announce: None }
    }

    /// A 200 answer carrying the whole body.
    pub fn ok(body: &[u8]) -> Reply {
        Reply::with(200, body)
    }

    /// A 206 answer for a range that starts at `from` of a file of `total` bytes. Inputs: the bytes from there on, the start and the file size. Returns the reply.
    pub fn partial(rest: &[u8], from: usize, total: usize) -> Reply {
        let mut reply = Reply::with(206, rest);
        reply.headers.push(("Content-Range", format!("bytes {from}-{}/{total}", total - 1)));
        reply
    }

    /// A redirect to `location`. Input: the target. Returns the reply.
    pub fn redirect(location: &str) -> Reply {
        let mut reply = Reply::with(302, b"");
        reply.headers.push(("Location", location.to_string()));
        reply
    }

    /// The same reply that claims to be longer than the bytes it sends, then closes: a dropped connection. Input: how many bytes arrive. Returns the reply.
    pub fn cut_at(mut self, sent: usize) -> Reply {
        self.announce = Some(self.body.len());
        self.body.truncate(sent);
        self
    }
}

/// A running server; it stops when dropped.
pub struct Server {
    pub url: String,
    hits: Arc<Mutex<Vec<Request>>>,
    stop: Arc<AtomicBool>,
    addr: SocketAddr,
    thread: Option<JoinHandle<()>>,
}

impl Server {
    /// Starts listening on a free loopback port. Input: the handler that answers each request. Returns the server.
    pub fn start(handler: impl Fn(&Request) -> Reply + Send + Sync + 'static) -> Server {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
        let addr = listener.local_addr().expect("address");
        let (hits, stop, handler) = (Arc::new(Mutex::new(Vec::new())), Arc::new(AtomicBool::new(false)), Arc::new(handler));
        let (seen, halt) = (hits.clone(), stop.clone());
        let thread = std::thread::spawn(move || {
            for stream in listener.incoming() {
                if halt.load(Ordering::Acquire) {
                    return;
                }
                let Ok(stream) = stream else { continue };
                let (seen, handler) = (seen.clone(), handler.clone());
                std::thread::spawn(move || answer(stream, &seen, &*handler));
            }
        });
        Server { url: format!("http://127.0.0.1:{}", addr.port()), hits, stop, addr, thread: Some(thread) }
    }

    /// Every request received so far, in order.
    pub fn requests(&self) -> Vec<Request> {
        self.hits.lock().expect("hits").clone()
    }
}

impl Drop for Server {
    /// Stops the accept loop by waking it with one last connection.
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let _ = TcpStream::connect(self.addr);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Reads one request from the stream, records it and writes the handler's reply. Inputs: the connection, the record and the handler.
fn answer(stream: TcpStream, seen: &Mutex<Vec<Request>>, handler: &(dyn Fn(&Request) -> Reply + Send + Sync)) {
    let mut reader = BufReader::new(&stream);
    let mut first = String::new();
    if reader.read_line(&mut first).unwrap_or(0) == 0 {
        return;
    }
    let path = first.split_whitespace().nth(1).unwrap_or("/").to_string();
    let mut range = None;
    let mut if_range = None;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 || line.trim().is_empty() {
            break;
        }
        if let Some(value) = line.to_ascii_lowercase().strip_prefix("range:") {
            range = Some(value.trim().to_string());
        }
        if line.to_ascii_lowercase().starts_with("if-range:") {
            if_range = line.split_once(':').map(|(_, v)| v.trim().to_string());
        }
    }
    let request = Request { path, range, if_range };
    seen.lock().expect("hits").push(request.clone());
    let reply = handler(&request);
    let mut head =
        format!("HTTP/1.1 {} X\r\nContent-Length: {}\r\nConnection: close\r\n", reply.status, reply.announce.unwrap_or(reply.body.len()));
    for (name, value) in &reply.headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    let mut out = &stream;
    let _ = out.write_all(head.as_bytes()).and_then(|()| out.write_all(b"\r\n")).and_then(|()| out.write_all(&reply.body));
    let _ = out.flush();
}

/// Deterministic bytes that are not a repeating block, so a wrong offset shows up in the hash. Input: length. Returns the bytes.
pub fn data(len: usize) -> Vec<u8> {
    (0..len).map(|i| ((i * 31 + i / 7) % 251) as u8).collect()
}

/// A model file entry (leaked for the test, since entries are `'static`) whose size and SHA-256 describe `bytes`. Inputs: file name and content. Returns the entry.
pub fn file_of(name: &str, bytes: &[u8]) -> &'static ModelFile {
    let sha256: &'static str = Box::leak(format!("{:x}", Sha256::digest(bytes)).into_boxed_str());
    let name: &'static str = Box::leak(name.to_string().into_boxed_str());
    Box::leak(Box::new(ModelFile { name, size: bytes.len() as u64, sha256 }))
}
