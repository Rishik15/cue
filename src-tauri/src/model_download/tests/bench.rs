//! File: model_download/tests/bench.rs
//! Purpose: A download bench run by `scripts/lab/download-bench.ps1`, ignored in normal test runs: it fetches one pinned file from the real network through `HttpNet`, times the stages seen from outside the transfer, then idles so the script can check that nothing is left running.
//! Contents: Timing — a `Net` wrapper that times the open and the first byte and counts bytes on the wire; download_bench — the ignored test (reads CUE_BENCH_URL, CUE_BENCH_SHA256, CUE_BENCH_SIZE and CUE_BENCH_DIR).

use std::io::Read;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::model_download::ctl::Ctl;
use crate::model_download::http::HttpNet;
use crate::model_download::net::{Fetched, Net};
use crate::model_download::state::{Manager, Sizes};
use crate::model_download::transfer::{fetch_retrying, Stop};
use crate::speech::catalog::ModelFile;

/// Times what a download asks of the network: each open, the wait for the first body byte, and every byte read.
struct Timing {
    inner: HttpNet,
    opens: AtomicU64,
    wire: Arc<AtomicU64>,
    open_ms: Mutex<Vec<u128>>,
    first_byte_ms: Arc<Mutex<Option<u128>>>,
}

/// Counts the bytes read through it and notes when the first one arrived, measured from `since`.
struct Counted {
    inner: Box<dyn Read + Send>,
    wire: Arc<AtomicU64>,
    first: Arc<Mutex<Option<u128>>>,
    since: Instant,
}

impl Read for Counted {
    /// Reads from the real body and records the bytes and the first-byte time.
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        if n > 0 {
            self.wire.fetch_add(n as u64, Ordering::Relaxed);
            self.first.lock().expect("first").get_or_insert(self.since.elapsed().as_millis());
        }
        Ok(n)
    }
}

impl Net for Timing {
    /// Opens through the real network and wraps the body so it is timed and counted.
    fn open(&self, url: &str, have: u64, validator: Option<&str>, ctl: &Ctl) -> Result<Option<Fetched>, Stop> {
        let started = Instant::now();
        self.opens.fetch_add(1, Ordering::Relaxed);
        let opened = self.inner.open(url, have, validator, ctl)?;
        self.open_ms.lock().expect("open_ms").push(started.elapsed().as_millis());
        Ok(opened.map(|f| Fetched {
            body: Box::new(Counted { inner: f.body, wire: self.wire.clone(), first: self.first_byte_ms.clone(), since: started }),
            ..f
        }))
    }
}

/// Fetches the pinned file, prints `BENCH key=value` lines for the script, hashes the finished file once more to time the verify pass, then idles for five seconds.
#[test]
#[ignore = "needs the network and CUE_BENCH_* settings; run by scripts/lab/download-bench.ps1"]
fn download_bench() {
    let var = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("{k} is not set"));
    let (url, sha256, size, dir) = (var("CUE_BENCH_URL"), var("CUE_BENCH_SHA256"), var("CUE_BENCH_SIZE"), var("CUE_BENCH_DIR"));
    let file: &'static ModelFile = Box::leak(Box::new(ModelFile {
        name: "bench.bin",
        size: size.parse().expect("CUE_BENCH_SIZE is a byte count"),
        sha256: Box::leak(sha256.into_boxed_str()),
    }));
    let manager: &'static Manager = Box::leak(Box::new(Manager::new()));
    let job = manager.admit("bench", "speech", Sizes { total: file.size, claims: vec![] }, &|| u64::MAX).expect("admitted");
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).expect("bench dir");
    let _ = std::fs::remove_file(dir.join(file.name));
    let net = Timing {
        inner: HttpNet,
        opens: AtomicU64::new(0),
        wire: Arc::new(AtomicU64::new(0)),
        open_ms: Mutex::new(Vec::new()),
        first_byte_ms: Arc::new(Mutex::new(None)),
    };
    println!("BENCH start");
    let (started, last) = (Instant::now(), Mutex::new(Instant::now()));
    let outcome = fetch_retrying(&net, &url, file, &dir, &job.ctl, &mut |_| *last.lock().expect("last") = Instant::now());
    let total = started.elapsed();
    let tail = last.lock().expect("last").elapsed();
    println!("BENCH ok={}", outcome.is_ok());
    println!("BENCH fetch_ms={}", total.as_millis());
    println!("BENCH tail_ms={} (fsync and rename after the last byte)", tail.as_millis());
    println!("BENCH open_ms={:?}", net.open_ms.lock().expect("open_ms"));
    println!("BENCH first_byte_ms={:?}", net.first_byte_ms.lock().expect("first"));
    println!("BENCH retries={}", net.opens.load(Ordering::Relaxed).saturating_sub(1));
    println!("BENCH wire_bytes={}", net.wire.load(Ordering::Relaxed));
    let verify = Instant::now();
    let again = fetch_retrying(&net, &url, file, &dir, &job.ctl, &mut |_| {});
    println!("BENCH verify_ms={} ok={}", verify.elapsed().as_millis(), again.is_ok());
    let _ = std::fs::remove_file(dir.join(file.name));
    println!("BENCH idle-start");
    std::thread::sleep(Duration::from_secs(5));
    println!("BENCH done");
}
