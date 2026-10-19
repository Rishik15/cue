//! Purpose: Download a model (speech or text): resumable, verified, cancellable, with progress events. One short-lived thread per download.
//! Contents: Wanted — one file to fetch (requests and redirects are in `download_http`); start — begin a download; cancel / is_active — control and query; run / fetch / fetch_retrying — per model and per file work (a dropped connection is retried with growing pauses).
//! A file is written as `<name>.partial`, resumed with an HTTP Range request, hashed while it is read, and only renamed to its real name
//! when the SHA-256 matches, so a file with its final name is always complete and genuine.

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter};

use super::catalog::ModelFile;
use crate::status;

#[path = "download_http.rs"]
mod http;
#[cfg(test)]
#[path = "download_test.rs"]
mod tests;

/// Pauses before each retry of a dropped download (seconds), about 90 in all.
const RETRY_SECONDS: [u64; 6] = [2, 4, 8, 16, 30, 30];

static ACTIVE: Mutex<Option<HashMap<&'static str, Arc<AtomicBool>>>> = Mutex::new(None);

/// A panic on a download thread must not leave downloads refused for good.
fn active_map() -> MutexGuard<'static, Option<HashMap<&'static str, Arc<AtomicBool>>>> {
    ACTIVE.lock().unwrap_or_else(PoisonError::into_inner)
}

#[derive(Serialize, Clone)]
struct Progress {
    id: &'static str,
    done: u64,
    total: u64,
}

#[derive(Serialize, Clone)]
struct Finished {
    id: &'static str,
    error: Option<String>,
}

#[derive(Debug)]
enum Stop {
    Cancelled,
    /// The connection dropped or the server was busy: the partial file is kept and the download is retried.
    Offline(String),
    Failed(String),
}

fn failed<E: std::fmt::Display>(what: &str) -> impl Fn(E) -> Stop + '_ {
    move |e| Stop::Failed(format!("{what}: {e}"))
}

/// Removes a download from `ACTIVE` when dropped.
struct Active(&'static str);

impl Drop for Active {
    fn drop(&mut self) {
        if let Some(map) = active_map().as_mut() {
            map.remove(self.0);
        }
    }
}

pub fn is_active(id: &str) -> bool {
    active_map().as_ref().is_some_and(|m| m.contains_key(id))
}

pub fn cancel(id: &str) {
    if let Some(flag) = active_map().as_ref().and_then(|m| m.get(id)) {
        flag.store(true, Ordering::Release);
    }
}

/// One file of a download: where it comes from, its pinned size and hash, and the folder it ends up in.
pub struct Wanted {
    pub url: String,
    pub file: &'static ModelFile,
    pub dir: PathBuf,
}

/// What runs once every file is in place (for example unpacking an archive).
pub type After = Box<dyn FnOnce(&AppHandle) -> Result<(), String> + Send>;

/// Fetches every file in `wanted`, then runs `after` (for example unpacking an archive). `id` names the model in progress events.
pub fn start(app: &AppHandle, id: &'static str, wanted: Vec<Wanted>, after: After) -> Result<(), String> {
    let cancelled = Arc::new(AtomicBool::new(false));
    {
        let mut active = active_map();
        let map = active.get_or_insert_with(HashMap::new);
        // One at a time: every download shares the voice detector file, and two writers would corrupt its partial file.
        if !map.is_empty() {
            return Err(
                if map.contains_key(id) { "This model is already downloading." } else { "Another model is still downloading." }.into()
            );
        }
        map.insert(id, cancelled.clone());
    }
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("download".into())
        .stack_size(256 * 1024)
        .spawn(move || {
            let active = Active(id); // released on every exit, a panic included, so the next download is never refused
            let work = || run(&app, id, &wanted, &cancelled).and_then(|()| after(&app).map_err(Stop::Failed));
            // A panic still ends in a Finished event, or the Models page would show the download as running forever.
            let outcome = crate::guard::contain(work).unwrap_or_else(|| Err(Stop::Failed("The download stopped unexpectedly.".into())));
            drop(active); // before the event: the page asks `is_active` when it hears it
            let error = match outcome {
                Err(Stop::Failed(e)) => Some(e),
                _ => None,
            };
            if let Some(e) = &error {
                status::report_in_app(&app, e);
            }
            let _ = app.emit("model-finished", Finished { id, error });
        })
        .map(|_| ());
    if spawned.is_err() {
        drop(Active(id)); // no thread will ever release it
    }
    spawned.map_err(|e| e.to_string())
}

fn run(app: &AppHandle, id: &'static str, wanted: &[Wanted], cancelled: &AtomicBool) -> Result<(), Stop> {
    let total: u64 = wanted.iter().map(|w| w.file.size).sum();
    let (mut base, mut last_percent) = (0u64, u64::MAX);
    for w in wanted {
        fs::create_dir_all(&w.dir).map_err(failed("Could not create the models folder"))?;
        let mut report = |file_bytes: u64| {
            let done = base + file_bytes;
            let percent = done * 100 / total;
            if percent != last_percent {
                last_percent = percent;
                let _ = app.emit("model-progress", Progress { id, done, total });
            }
        };
        fetch_retrying(&w.url, w.file, &w.dir, cancelled, &mut report)?;
        base += w.file.size;
    }
    Ok(())
}

/// Waits between attempts, in slices, so a cancel is noticed within a quarter of a second.
fn pause(wait: Duration, cancelled: &AtomicBool) -> Result<(), Stop> {
    let end = Instant::now() + wait;
    while Instant::now() < end {
        if cancelled.load(Ordering::Acquire) {
            return Err(Stop::Cancelled);
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    Ok(())
}

/// A dropped connection resumes from the partial file after a growing pause, like a browser: about 90 seconds without any progress and it gives up. Any progress starts the count again.
fn fetch_retrying(url: &str, file: &ModelFile, dir: &Path, cancelled: &AtomicBool, report: &mut dyn FnMut(u64)) -> Result<(), Stop> {
    let part = dir.join(format!("{}.partial", file.name));
    let size = || fs::metadata(&part).map_or(0, |m| m.len());
    let (mut tries, mut before) = (0, size());
    loop {
        let why = match fetch(url, file, dir, cancelled, report) {
            Err(Stop::Offline(why)) => why,
            done => return done,
        };
        let now = size();
        if now > before {
            (tries, before) = (0, now);
        }
        let Some(secs) = RETRY_SECONDS.get(tries) else { return Err(Stop::Failed(why)) };
        pause(Duration::from_secs(*secs), cancelled)?;
        tries += 1;
    }
}

fn hash_existing(path: &Path, limit: u64, cancelled: &AtomicBool) -> Result<(Sha256, u64), Stop> {
    let mut hasher = Sha256::new();
    let Ok(mut file) = File::open(path) else { return Ok((hasher, 0)) };
    let mut buf = vec![0u8; 64 * 1024];
    let mut have = 0u64;
    loop {
        if cancelled.load(Ordering::Acquire) {
            return Err(Stop::Cancelled);
        }
        let n = file.read(&mut buf).map_err(failed("Could not read the partial download"))?;
        if n == 0 || have + n as u64 > limit {
            break;
        }
        hasher.update(&buf[..n]);
        have += n as u64;
    }
    Ok((hasher, have))
}

fn fetch(url: &str, file: &ModelFile, dir: &Path, cancelled: &AtomicBool, report: &mut dyn FnMut(u64)) -> Result<(), Stop> {
    let target = dir.join(file.name);
    if fs::metadata(&target).is_ok_and(|m| m.len() == file.size) {
        report(file.size);
        return Ok(());
    }
    let part = dir.join(format!("{}.partial", file.name));
    if fs::metadata(&part).is_ok_and(|m| m.len() > file.size) {
        let _ = fs::remove_file(&part); // longer than the real file: cannot be a prefix of it, start over
    }
    let (mut hasher, mut have) = hash_existing(&part, file.size, cancelled)?;
    if have < file.size {
        have = download_body(url, file, &part, have, &mut hasher, cancelled, report)?;
    }
    if have < file.size {
        // The connection ended early: keep the partial so the next try resumes from it.
        return Err(Stop::Offline(format!("Download of {} was interrupted. Try again to resume.", file.name)));
    }
    if format!("{:x}", hasher.finalize()) != file.sha256 {
        let _ = fs::remove_file(&part);
        return Err(Stop::Failed(format!("{} failed verification. Try the download again.", file.name)));
    }
    fs::rename(&part, &target).map_err(failed("Could not save the model file"))
}

/// Streams the rest of the file into `part`; returns the total bytes now on disk. The server answers 206 when it honours the
/// range; a plain 200 means "from the start", so the partial is discarded.
fn download_body(
    url: &str,
    file: &ModelFile,
    part: &Path,
    mut have: u64,
    hasher: &mut Sha256,
    cancelled: &AtomicBool,
    report: &mut dyn FnMut(u64),
) -> Result<u64, Stop> {
    let response = match http::open(url, have)? {
        Some(r) => r,
        None => {
            // 416: the partial does not fit the file on the server. Start over, once.
            let _ = fs::remove_file(part);
            (*hasher, have) = (Sha256::new(), 0);
            http::open(url, 0)?.ok_or_else(|| Stop::Failed("The download server rejected the request.".into()))?
        }
    };
    let append = have > 0 && response.status() == 206;
    let resumed_at = response.header("Content-Range").is_some_and(|r| r.starts_with(&format!("bytes {have}-")));
    if append && !resumed_at {
        let _ = fs::remove_file(part);
        return Err(Stop::Failed("The download server resumed at the wrong place. Try again.".into()));
    }
    if !append {
        (*hasher, have) = (Sha256::new(), 0);
    }
    let out = OpenOptions::new()
        .create(true)
        .write(true)
        .append(append)
        .truncate(!append)
        .open(part)
        .map_err(failed("Could not write the model file"))?;
    copy_body(response.into_reader(), out, file.size, have, hasher, cancelled, report)
}

/// Writes the body to `out` (and the hash) until `size` bytes are on disk; returns how many are.
fn copy_body(
    mut body: impl Read,
    mut out: File,
    size: u64,
    mut have: u64,
    hasher: &mut Sha256,
    cancelled: &AtomicBool,
    report: &mut dyn FnMut(u64),
) -> Result<u64, Stop> {
    let mut buf = vec![0u8; 64 * 1024];
    while have < size {
        if cancelled.load(Ordering::Acquire) {
            return Err(Stop::Cancelled);
        }
        let n = body.read(&mut buf).map_err(|e| Stop::Offline(format!("Download interrupted: {e}")))?;
        if n == 0 {
            break;
        }
        let n = n.min((size - have) as usize);
        out.write_all(&buf[..n]).map_err(failed("Could not write the model file"))?;
        hasher.update(&buf[..n]);
        have += n as u64;
        report(have);
    }
    Ok(have)
}
