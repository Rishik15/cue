//! File: speech/mod.rs
//! Purpose: Speech to text. The recognizer lives in a separate worker process (same exe, `--speech-worker`) that is started when dictation
//! begins and killed after the idle time, so a loaded model costs memory only while it is useful and an engine crash never reaches the app.
//! Contents: on_partial — hear sentences as they are recognised; on_failed — hear that a recording can no longer be recognised; has_model — is anything installed; begin — start loading on hotkey down; feed / finish — stream the recording and collect its text; cancel — drop work in flight; evict — close a model to new loads and stop its worker before deletion;
//! models — Models page commands. `session` decides everything about a dictation session (pure), `pump` runs it on the speech thread (started on demand, gone when idle),
//! `process` is the child process, `protocol` the pipe format, `take` the clauses and unreleased audio of one dictation, `catalog` the model files (downloads are in `model_download`).

pub(crate) mod catalog;
pub(crate) mod catalog_stream;
mod events;
pub mod models;
mod online;
mod phrase;
pub(crate) mod process;
mod protocol;
mod pump;
mod session;
mod take;
pub mod worker;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Mutex, OnceLock, PoisonError};
use std::time::Duration;

use tauri::AppHandle;

use crate::slot::Slot;
use events::LOST;
use pump::{Msg, Shell};
pub use take::Failure;

/// The speech model a new install uses and the Models page recommends: the light one on a weak PC.
pub fn default_model() -> &'static str {
    if crate::platform::weak_machine() {
        catalog::LIGHT
    } else {
        catalog::DEFAULT
    }
}

/// Receives each sentence as it is recognised: dictation id, sentence number, text. Runs on the speech thread.
type PartialSink = Box<dyn Fn(u64, usize, &str) + Send + Sync>;
static PARTIAL: OnceLock<PartialSink> = OnceLock::new();

/// The speech thread's handle: empty until the first dictation and again after the thread retired.
static SLOT: Slot<Msg> = Slot::new();
static NEXT_JOB: AtomicU64 = AtomicU64::new(1);
/// The dictation being recorded (0: none), so `cancel` names the recording it drops instead of counting.
static OPEN: AtomicU64 = AtomicU64::new(0);
/// Why the speech thread could not start for a dictation (its id and the reason), kept so that dictation's `finish` says it once instead of a generic
/// message, and nobody reports it a second time.
static START_FAILED: Mutex<Option<(u64, String)>> = Mutex::new(None);

/// Starts the speech thread: it runs until nothing is loaded or in flight, then retires through the slot. Inputs: the app and the thread's channel.
fn start(app: &AppHandle, tx: Sender<Msg>, rx: Receiver<Msg>) -> std::io::Result<()> {
    let shell = Shell::new(app.clone(), tx);
    crate::guard::spawn("speech", 512 * 1024, move || pump::run(shell, rx, |rx| SLOT.retire(rx)))
}

/// Registers who hears about sentences while the user is still talking (the live caption). Set once at startup.
pub fn on_partial(sink: impl Fn(u64, usize, &str) + Send + Sync + 'static) {
    let _ = PARTIAL.set(Box::new(sink));
}

/// Receives the id of a recording that can no longer be recognised, so its owner ends it. Runs on the speech thread.
type FailedSink = Box<dyn Fn(u64) + Send + Sync>;
static FAILED: OnceLock<FailedSink> = OnceLock::new();

/// Registers who ends a recording whose engine failed or was unloaded. Set once at startup.
pub fn on_failed(sink: impl Fn(u64) + Send + Sync + 'static) {
    let _ = FAILED.set(Box::new(sink));
}

/// Tells the registered owner that recording `id` cannot be recognised. Input: the dictation id.
fn emit_failed(id: u64) {
    if let Some(sink) = FAILED.get() {
        sink(id);
    }
}

/// Shows a recognised clause through the registered sink. Inputs: the dictation id, the clause number and its text.
fn emit_partial(id: u64, index: usize, text: &str) {
    if let Some(sink) = PARTIAL.get() {
        sink(id, index, text);
    }
}

/// Whether any speech model is installed: a few file-size checks, cheap enough for every hotkey press.
pub fn has_model(app: &AppHandle) -> bool {
    catalog::all().any(|m| catalog::installed(app, m))
}

/// Hotkey went down: the worker starts loading now, while the user is still speaking. Returns the id of this dictation. A speech thread that
/// cannot start is not reported here: the dictation ends with that reason as its error, which its owner shows (once).
/// Thread: the shortcut effects thread (`dictation::on_hotkey` runs there, never on the keyboard hook thread), so starting the speech thread
/// and waiting for the short `Slot` lock are allowed.
pub fn begin(app: &AppHandle) -> u64 {
    let id = NEXT_JOB.fetch_add(1, Ordering::Relaxed);
    OPEN.store(id, Ordering::Release);
    *START_FAILED.lock().unwrap_or_else(PoisonError::into_inner) = None;
    if let Err((_, why)) = SLOT.send(Msg::Begin(id), |tx, rx| start(app, tx, rx)) {
        *START_FAILED.lock().unwrap_or_else(PoisonError::into_inner) = Some((id, format!("Cue could not start its speech engine: {why}")));
    }
    id
}

/// The message for an End that found no speech thread: the reason it could not start for dictation `id`, else that the recording was lost.
fn end_error(id: u64) -> String {
    let mut failed = START_FAILED.lock().unwrap_or_else(PoisonError::into_inner);
    match failed.take_if(|(failed_id, _)| *failed_id == id) {
        Some((_, why)) => why,
        None => LOST.to_string(),
    }
}

/// Audio for dictation `id`, 16 kHz mono. Called from the audio thread every 200 ms while recording.
pub fn feed(id: u64, samples: Vec<f32>) {
    let _ = SLOT.try_send(Msg::Audio(id, samples));
}

/// The user stopped. `done` runs on the speech thread with the whole text or a message the user can act on, exactly once, and not at all for
/// cancelled work. When the speech thread is gone it runs here with an error.
pub fn finish(id: u64, done: impl FnOnce(Result<String, Failure>) + Send + 'static) {
    let _ = OPEN.compare_exchange(id, 0, Ordering::AcqRel, Ordering::Acquire);
    if let Err(Msg::End(_, done)) = SLOT.try_send(Msg::End(id, Box::new(done))) {
        done(Err(end_error(id).into()));
    }
}

/// Escape pressed: the recording in progress is dropped; dictations already stopped are still delivered. Returns whether there was a recording to cancel.
pub fn cancel() -> bool {
    let id = OPEN.swap(0, Ordering::AcqRel);
    if id == 0 {
        return false;
    }
    let _ = SLOT.try_send(Msg::Cancel(id));
    true
}

/// Speech models being deleted: no dictation loads them until their `Eviction` is dropped.
static DELETING: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());

/// Whether any model is being deleted.
fn is_deleting() -> bool {
    !DELETING.lock().unwrap_or_else(PoisonError::into_inner).is_empty()
}

/// Whether model `id` is being deleted, so a dictation must not load it.
fn is_deleting_model(id: &str) -> bool {
    DELETING.lock().unwrap_or_else(PoisonError::into_inner).contains(&id)
}

/// Proof that model `id` is closed to new loads and no worker holds its files; dropping it reopens the model.
pub struct Eviction {
    id: &'static str,
    /// Whether a worker was stopped, which drops work in flight.
    pub stopped: bool,
}

impl Drop for Eviction {
    fn drop(&mut self) {
        let mut deleting = DELETING.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(at) = deleting.iter().position(|d| *d == self.id) {
            deleting.swap_remove(at);
        }
    }
}

/// Closes model `id` to new loads, then stops the worker if it runs that model and waits for its process to exit, so the files can be deleted.
/// A worker on another model, and any recording on it, is left alone. Returns the eviction to hold until the files are gone, or why the worker
/// did not exit in time (the delete must then not go ahead).
pub fn evict(id: &'static str) -> Result<Eviction, String> {
    DELETING.lock().unwrap_or_else(PoisonError::into_inner).push(id);
    let mut eviction = Eviction { id, stopped: false };
    let (ack, done) = mpsc::channel();
    if SLOT.try_send(Msg::Evict(id, ack)).is_ok() {
        eviction.stopped = done
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| "The speech engine did not stop in time, so the model was not deleted. Try again.".to_string())?;
    }
    Ok(eviction)
}

#[cfg(test)]
mod session_tests;
#[cfg(test)]
mod tests;
