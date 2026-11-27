//! File: model_download/transfer.rs
//! Purpose: Fetch one model file: resumable, verified and cancellable. A file is written as `<name>.partial`, resumed with an HTTP Range request, hashed while it is read, flushed to disk
//! and only renamed to its real name when the SHA-256 matches, so a file with its final name is always complete and genuine.
//! Contents: Stop — why a fetch ended early (Busy: the server named a pause); validator_path / stored_validator / store_validator / discard — the strong ETag kept beside a partial; Account — the cancel object plus the disk-book key of the file; fetch_retrying — one file with retries (a dropped connection is retried with growing pauses); fetch / download_body / copy_body / pump — one attempt.
//! shortcut: a partial that was started without a strong ETag never gets one, so its resume is guarded by the final SHA-256 only; upgrade: store the ETag of a 206 answer once the bench shows resumes of such partials matter.
//! shortcut: a plain-file model publishes by renaming each verified file, which is not a commit point; a cancel arriving after the last rename but before the download concludes is reported as cancelled although the files are complete; upgrade: commit before the last file's rename when the model has no installer.

use crate::lock::{lock, wait_timeout_while};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use sha2::{Digest, Sha256};

use super::ctl::{Ctl, Phase};
use super::interrupt::is_helper_panic;
use super::ledger::path_key;
use super::net::{Fetched, Net};
use super::resume;
use crate::speech::catalog::ModelFile;

/// Pauses before each retry of a dropped download (seconds), about 90 in all.
const RETRY_SECONDS: [u64; 6] = [2, 4, 8, 16, 30, 30];

/// Why a fetch ended early.
#[derive(Debug)]
pub(super) enum Stop {
    /// The user (or Quit) cancelled the download.
    Cancelled,
    /// The connection dropped or the server was busy: the partial file is kept and the download is retried.
    Offline(String),
    /// The server asked for a pause (429/503): the partial file is kept and the download is retried after the wait it named, if it named one.
    Busy(String, Option<Duration>),
    /// The download cannot succeed by retrying; the text is shown to the user.
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

/// Sleeps for `wait` on a condition variable that a cancel signals through an interrupt, so a cancel ends the wait at once and the wait costs no polling. Returns Cancelled when the download was cancelled.
fn pause(wait: Duration, ctl: &Ctl) -> Result<(), Stop> {
    let bell = Arc::new((Mutex::new(false), Condvar::new()));
    let ring = bell.clone();
    let id = ctl.register(Box::new(move || {
        *lock(&ring.0) = true;
        ring.1.notify_all();
    }));
    let (rung, cond) = &*bell;
    let guard = lock(rung);
    drop(wait_timeout_while(cond, guard, wait, |rung| !*rung));
    ctl.unregister(id);
    if ctl.is_cancelled() {
        return Err(Stop::Cancelled);
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
    crate::fsutil::commit(&part, &target).map_err(failed("Could not save the model file"))?;
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
            acct.lose(have);
            discard(part);
            return Err(Stop::Failed(why.into()));
        }
        if changed {
            acct.lose(have);
            discard(part);
            return Err(Stop::Failed("The file changed on the download server. Try again.".into()));
        }
    }
    let append = have > 0 && response.status == 206;
    if !append {
        acct.lose(have);
        (*hasher, have) = (Sha256::new(), 0);
    }
    let out = OpenOptions::new()
        .create(true)
        .write(true)
        .append(append)
        .truncate(!append)
        .open(part)
        .map_err(failed("Could not write the model file"))?;
    if !append {
        store_validator(part, response.etag.as_deref());
    }
    copy_body(response.body, out, file.size, have, hasher, acct, report)
}

/// Writes the body to `out` (and the hash) until `size` bytes are on disk; returns how many are. Every read goes straight to the file and is synced even when the body ends early or fails, so the partial on disk is exactly what a resume hashes.
fn copy_body(
    body: impl Read,
    mut out: File,
    size: u64,
    have: u64,
    hasher: &mut Sha256,
    acct: &Account,
    report: &mut dyn FnMut(u64),
) -> Result<u64, Stop> {
    let pumped = pump(body, &mut out, size, have, hasher, acct, report);
    let flushed = out.sync_all().map_err(failed("Could not write the model file"));
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
            } else if is_helper_panic(&e) {
                Stop::Failed(e.to_string())
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
