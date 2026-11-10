//! File: model_download/transfer.rs
//! Purpose: Fetch one model file: resumable, verified and cancellable. A file is written as `<name>.partial`, resumed with an HTTP Range request, hashed while it is read, flushed to disk
//! and only renamed to its real name when the SHA-256 matches, so a file with its final name is always complete and genuine.
//! Contents: Stop — why a fetch ended early (Busy: the server named a pause); validator_path / stored_validator / store_validator / discard — the strong ETag kept beside a partial; Account — the cancel object plus the disk-book key of the file; fetch_retrying — one file with retries (a dropped connection is retried with growing pauses); fetch / download_body / copy_body / pump — one attempt (writes are gathered into `WRITE_BATCH` blocks).
//! shortcut: a plain-file model publishes by renaming each verified file, which is not a commit point; a cancel arriving after the last rename but before the download concludes is reported as cancelled although the files are complete; upgrade: commit before the last file's rename when the model has no installer.

use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Read, Write};
use std::path::Path;
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};

use super::ctl::{Ctl, Phase};
use super::ledger::path_key;
use super::net::{Fetched, Net};
use super::resume;
use crate::speech::catalog::ModelFile;

/// Bytes gathered before one write to the file: one buffer per download, no thread. 0 writes every read straight through (the way to bench without coalescing).
const WRITE_BATCH: usize = 512 * 1024;

/// Pauses before each retry of a dropped download (seconds), about 90 in all.
const RETRY_SECONDS: [u64; 6] = [2, 4, 8, 16, 30, 30];

#[derive(Debug)]
pub(super) enum Stop {
    Cancelled,
    /// The connection dropped or the server was busy: the partial file is kept and the download is retried.
    Offline(String),
    /// The server asked for a pause (429/503): the partial file is kept and the download is retried after the wait it named, if it named one.
    Busy(String, Option<Duration>),
    Failed(String),
}

/// The cancel object of a download together with the key of the file being fetched in the disk books.
struct Account<'a> {
    net: &'a dyn Net,
    ctl: &'a Ctl,
    key: String,
}

impl Account<'_> {
    /// Notes that `bytes` already on disk are about to be thrown away and written again.
    fn lose(&self, bytes: u64) {
        self.ctl.lose(&self.key, bytes);
    }

    /// Notes that `bytes` more were written.
    fn wrote(&self, bytes: u64) {
        self.ctl.wrote(&self.key, bytes);
    }
}

/// Wraps an error as a failed fetch with `what` in front of it.
pub(super) fn failed<E: std::fmt::Display>(what: &str) -> impl Fn(E) -> Stop + '_ {
    move |e| Stop::Failed(format!("{what}: {e}"))
}

/// Waits between attempts, in slices, so a cancel is noticed within a quarter of a second.
fn pause(wait: Duration, ctl: &Ctl) -> Result<(), Stop> {
    let end = Instant::now() + wait;
    while Instant::now() < end {
        if ctl.is_cancelled() {
            return Err(Stop::Cancelled);
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    Ok(())
}

/// A dropped connection resumes from the partial file after a growing pause, like a browser: about 90 seconds without any progress and it gives up. Any progress starts the count again.
pub(super) fn fetch_retrying(
    net: &dyn Net,
    url: &str,
    file: &ModelFile,
    dir: &Path,
    ctl: &Ctl,
    report: &mut dyn FnMut(u64),
) -> Result<(), Stop> {
    let part = dir.join(format!("{}.partial", file.name));
    let size = || fs::metadata(&part).map_or(0, |m| m.len());
    let (mut tries, mut before) = (0, size());
    loop {
        let (why, asked) = match fetch(net, url, file, dir, ctl, report) {
            Err(Stop::Offline(why)) => (why, None),
            Err(Stop::Busy(why, wait)) => (why, wait),
            done => return done,
        };
        let now = size();
        if now > before {
            (tries, before) = (0, now);
        }
        let Some(secs) = RETRY_SECONDS.get(tries) else { return Err(Stop::Failed(why)) };
        pause(asked.unwrap_or(Duration::from_secs(*secs)), ctl)?;
        tries += 1;
    }
}

/// Hashes the first `limit` bytes of a partial file; returns the hasher and how many bytes it read (0 when there is no file).
fn hash_existing(path: &Path, limit: u64, ctl: &Ctl) -> Result<(Sha256, u64), Stop> {
    let mut hasher = Sha256::new();
    let Ok(mut file) = File::open(path) else { return Ok((hasher, 0)) };
    let mut buf = vec![0u8; 64 * 1024];
    let mut have = 0u64;
    loop {
        if ctl.is_cancelled() {
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

/// One attempt at `file`: reuses a complete file, resumes a partial one, verifies the hash and renames. Returns Offline when the connection ended early.
fn fetch(net: &dyn Net, url: &str, file: &ModelFile, dir: &Path, ctl: &Ctl, report: &mut dyn FnMut(u64)) -> Result<(), Stop> {
    let acct = Account { net, ctl, key: path_key(dir, file.name) };
    let target = dir.join(file.name);
    if fs::metadata(&target).is_ok_and(|m| m.len() == file.size) {
        ctl.set_phase(Phase::Verifying);
        let (hasher, _) = hash_existing(&target, file.size, ctl)?;
        if format!("{:x}", hasher.finalize()) == file.sha256 {
            report(file.size);
            return Ok(());
        }
        acct.lose(file.size);
        fs::remove_file(&target).map_err(failed("Could not replace a damaged model file"))?;
    }
    let part = dir.join(format!("{}.partial", file.name));
    if fs::metadata(&part).is_ok_and(|m| m.len() > file.size) {
        acct.lose(file.size);
        discard(&part);
    }
    if fs::metadata(&part).is_ok_and(|m| m.len() > 0) {
        ctl.set_phase(Phase::Verifying);
    }
    let (mut hasher, mut have) = hash_existing(&part, file.size, ctl)?;
    if have < file.size {
        ctl.set_phase(Phase::Fetching);
        have = download_body(url, file, &part, have, &mut hasher, &acct, report)?;
    }
    if have < file.size {
        return Err(Stop::Offline(format!("Download of {} was interrupted. Try again to resume.", file.name)));
    }
    if format!("{:x}", hasher.finalize()) != file.sha256 {
        acct.lose(file.size);
        discard(&part);
        return Err(Stop::Failed(format!("{} failed verification. Try the download again.", file.name)));
    }
    if ctl.is_cancelled() {
        return Err(Stop::Cancelled);
    }
    fs::rename(&part, &target).map_err(failed("Could not save the model file"))?;
    let _ = fs::remove_file(validator_path(&part));
    Ok(())
}

/// Where the strong ETag of a partial file is kept, next to it.
fn validator_path(part: &Path) -> std::path::PathBuf {
    let mut name = part.as_os_str().to_os_string();
    name.push(".etag");
    name.into()
}

/// Removes a partial file and the validator kept for it.
fn discard(part: &Path) {
    let _ = fs::remove_file(part);
    let _ = fs::remove_file(validator_path(part));
}

/// The strong ETag the partial file was started under; None when there is none or no partial to guard.
fn stored_validator(part: &Path) -> Option<String> {
    let tag = fs::read_to_string(validator_path(part)).ok()?;
    (resume::strong(&tag) && fs::metadata(part).is_ok_and(|m| m.len() > 0)).then(|| tag.trim().to_string())
}

/// Keeps `etag` for the partial file when it is strong, otherwise forgets any older one.
fn store_validator(part: &Path, etag: Option<&str>) {
    match etag.filter(|t| resume::strong(t)) {
        Some(tag) => {
            let _ = fs::write(validator_path(part), tag.trim());
        }
        None => {
            let _ = fs::remove_file(validator_path(part));
        }
    }
}

/// Streams the rest of the file into `part`; returns the total bytes now on disk. The server answers 206 when it honours the
/// range; a plain 200 means "from the start", so the partial is discarded.
fn download_body(
    url: &str,
    file: &ModelFile,
    part: &Path,
    mut have: u64,
    hasher: &mut Sha256,
    acct: &Account,
    report: &mut dyn FnMut(u64),
) -> Result<u64, Stop> {
    let validator = if have > 0 { stored_validator(part) } else { None };
    let response: Fetched = match acct.net.open(url, have, validator.as_deref(), acct.ctl)? {
        Some(r) => r,
        None => {
            acct.lose(have);
            discard(part);
            (*hasher, have) = (Sha256::new(), 0);
            acct.net.open(url, 0, None, acct.ctl)?.ok_or_else(|| Stop::Failed("The download server rejected the request.".into()))?
        }
    };
    if !resume::identity(response.encoding.as_deref()) {
        return Err(Stop::Failed("The download server re-encoded the file. Try again later.".into()));
    }
    if response.status == 206 {
        let changed = validator.as_deref().zip(response.etag.as_deref()).is_some_and(|(kept, now)| kept != now.trim());
        if let Err(why) = resume::check_range(response.content_range.as_deref(), have, file.size) {
            discard(part);
            return Err(Stop::Failed(why.into()));
        }
        if changed {
            discard(part);
            return Err(Stop::Failed("The file changed on the download server. Try again.".into()));
        }
    }
    let append = have > 0 && response.status == 206;
    if !append {
        acct.lose(have);
        (*hasher, have) = (Sha256::new(), 0);
        store_validator(part, response.etag.as_deref());
    }
    let out = OpenOptions::new()
        .create(true)
        .write(true)
        .append(append)
        .truncate(!append)
        .open(part)
        .map_err(failed("Could not write the model file"))?;
    copy_body(response.body, out, file.size, have, hasher, acct, report)
}

/// Writes the body to `out` through a `WRITE_BATCH` buffer (and the hash) until `size` bytes are on disk; returns how many are. Whatever was read is flushed to the file even when the body ends early or fails, so the partial on disk is exactly what a resume hashes.
fn copy_body(
    body: impl Read,
    out: File,
    size: u64,
    have: u64,
    hasher: &mut Sha256,
    acct: &Account,
    report: &mut dyn FnMut(u64),
) -> Result<u64, Stop> {
    let mut out = BufWriter::with_capacity(WRITE_BATCH, out);
    let pumped = pump(body, &mut out, size, have, hasher, acct, report);
    let flushed = out.flush().and_then(|()| out.get_ref().sync_all()).map_err(failed("Could not write the model file"));
    let have = pumped?;
    flushed?;
    Ok(have)
}

/// Reads the body into `out` and the hash until `size` bytes are held or the body ends; returns how many bytes are held in all.
fn pump(
    mut body: impl Read,
    out: &mut impl Write,
    size: u64,
    mut have: u64,
    hasher: &mut Sha256,
    acct: &Account,
    report: &mut dyn FnMut(u64),
) -> Result<u64, Stop> {
    let mut buf = vec![0u8; 64 * 1024];
    while have < size {
        if acct.ctl.is_cancelled() {
            return Err(Stop::Cancelled);
        }
        let n = body.read(&mut buf).map_err(|e| {
            if acct.ctl.is_cancelled() {
                Stop::Cancelled
            } else {
                Stop::Offline(format!("Download interrupted: {e}"))
            }
        })?;
        if n == 0 {
            break;
        }
        let n = n.min((size - have) as usize);
        out.write_all(&buf[..n]).map_err(failed("Could not write the model file"))?;
        acct.wrote(n as u64);
        hasher.update(&buf[..n]);
        have += n as u64;
        report(have);
    }
    Ok(have)
}
