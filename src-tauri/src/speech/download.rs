//! Purpose: Download a model (speech or text): resumable, verified, cancellable, with progress events. One short-lived thread per download.
//! Contents: Wanted — one file to fetch; start — begin a download; cancel / is_active — control and query; run / fetch — per model and per file work.
//! A file is written as `<name>.partial`, resumed with an HTTP Range request, hashed while it is read, and only renamed to its real name
//! when the SHA-256 matches, so a file with its final name is always complete and genuine.

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter};

use super::catalog::ModelFile;
use crate::status;

#[cfg(test)]
#[path = "download_test.rs"]
mod tests;

static ACTIVE: Mutex<Option<HashMap<&'static str, Arc<AtomicBool>>>> = Mutex::new(None);

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
    Failed(String),
}

fn failed<E: std::fmt::Display>(what: &str) -> impl Fn(E) -> Stop + '_ {
    move |e| Stop::Failed(format!("{what}: {e}"))
}

/// Removes a download from `ACTIVE` when dropped.
struct Active(&'static str);

impl Drop for Active {
    fn drop(&mut self) {
        if let Some(map) = ACTIVE.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
            map.remove(self.0);
        }
    }
}

pub fn is_active(id: &str) -> bool {
    ACTIVE.lock().unwrap().as_ref().is_some_and(|m| m.contains_key(id))
}

pub fn cancel(id: &str) {
    if let Some(flag) = ACTIVE.lock().unwrap().as_ref().and_then(|m| m.get(id)) {
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
        let mut active = ACTIVE.lock().unwrap();
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
            let outcome = run(&app, id, &wanted, &cancelled).and_then(|()| after(&app).map_err(Stop::Failed));
            drop(active); // before the event: the page asks `is_active` when it hears it
            let error = match outcome {
                Err(Stop::Failed(e)) => Some(e),
                _ => None,
            };
            if let Some(e) = &error {
                status::report(&app, e);
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
        fetch(&w.url, w.file, &w.dir, cancelled, &mut report)?;
        base += w.file.size;
    }
    Ok(())
}

fn hash_existing(path: &Path, limit: u64) -> Result<(Sha256, u64), Stop> {
    let mut hasher = Sha256::new();
    let Ok(mut file) = File::open(path) else { return Ok((hasher, 0)) };
    let mut buf = vec![0u8; 64 * 1024];
    let mut have = 0u64;
    loop {
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
    let (mut hasher, mut have) = hash_existing(&part, file.size)?;
    if have < file.size {
        have = download_body(url, file, &part, have, &mut hasher, cancelled, report)?;
    }
    if have < file.size {
        // The connection ended early: keep the partial so the next try resumes from it.
        return Err(Stop::Failed(format!("Download of {} was interrupted. Try again to resume.", file.name)));
    }
    if format!("{:x}", hasher.finalize()) != file.sha256 {
        let _ = fs::remove_file(&part);
        return Err(Stop::Failed(format!("{} failed verification. Try the download again.", file.name)));
    }
    fs::rename(&part, &target).map_err(failed("Could not save the model file"))
}

/// Downloads go over HTTPS only. The tests talk to a loopback server over plain HTTP, so the test build also accepts that address (and nothing else).
fn secure(url: &str) -> bool {
    url.starts_with("https://") || (cfg!(test) && url.starts_with("http://127.0.0.1"))
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
    if !secure(url) {
        return Err(Stop::Failed("Models are only downloaded over HTTPS.".into()));
    }
    let agent = ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(15)).timeout_read(Duration::from_secs(30)).build();
    let mut request = agent.get(url);
    if have > 0 {
        request = request.set("Range", &format!("bytes={have}-"));
    }
    let response = request.call().map_err(|e| Stop::Failed(format!("Download failed: {e}. Check your connection and try again.")))?;
    if !secure(response.get_url()) {
        return Err(Stop::Failed("The download was redirected to an insecure address, so it was stopped.".into()));
        // a redirect must not downgrade to HTTP
    }
    let append = have > 0 && response.status() == 206;
    if !append {
        (*hasher, have) = (Sha256::new(), 0);
    }
    let mut out = OpenOptions::new()
        .create(true)
        .write(true)
        .append(append)
        .truncate(!append)
        .open(part)
        .map_err(failed("Could not write the model file"))?;
    let mut body = response.into_reader();
    let mut buf = vec![0u8; 64 * 1024];
    while have < file.size {
        if cancelled.load(Ordering::Acquire) {
            return Err(Stop::Cancelled);
        }
        let n = body.read(&mut buf).map_err(failed("Download interrupted"))?;
        if n == 0 {
            break;
        }
        let n = n.min((file.size - have) as usize);
        out.write_all(&buf[..n]).map_err(failed("Could not write the model file"))?;
        hasher.update(&buf[..n]);
        have += n as u64;
        report(have);
    }
    Ok(have)
}
