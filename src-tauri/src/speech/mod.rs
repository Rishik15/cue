//! File: speech/mod.rs
//! Purpose: Speech to text. The recognizer lives in a separate worker process (same exe, `--speech-worker`) that is started when dictation
//! begins and killed after the idle time, so a loaded model costs memory only while it is useful and an engine crash never reaches the app.
//! Contents: on_partial — hear sentences as they are recognised; has_model — is anything installed; begin — start loading on hotkey down; feed / finish — stream the recording and collect its text; cancel — drop work in flight; unload — stop the worker;
//! models — Models page commands. `session` decides everything about a dictation session (pure), `pump` runs it on the speech thread (started on demand, gone when idle),
//! `process` is the child process, `protocol` the pipe format, `take` the clauses and unreleased audio of one dictation, `catalog` the model files (downloads are in `model_download`).

pub(crate) mod catalog;
pub(crate) mod catalog_stream;
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
use std::sync::OnceLock;
use std::time::Duration;

use tauri::AppHandle;

use crate::slot::Slot;
use pump::{Msg, Shell};

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
const LOST: &str = "Cue lost track of this recording. Try dictating again.";

/// Starts the speech thread: it runs until nothing is loaded or in flight, then retires through the slot. Inputs: the app and the thread's channel.
fn start(app: &AppHandle, tx: Sender<Msg>, rx: Receiver<Msg>) -> std::io::Result<()> {
    let shell = Shell::new(app.clone(), tx);
    crate::guard::spawn("speech", 512 * 1024, move || pump::run(shell, rx, |rx| SLOT.retire(rx)))
}

/// Registers who hears about sentences while the user is still talking (the live caption). Set once at startup.
pub fn on_partial(sink: impl Fn(u64, usize, &str) + Send + Sync + 'static) {
    let _ = PARTIAL.set(Box::new(sink));
}

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
/// cannot start is reported, and the dictation then ends with an error instead of text.
pub fn begin(app: &AppHandle) -> u64 {
    let id = NEXT_JOB.fetch_add(1, Ordering::Relaxed);
    OPEN.store(id, Ordering::Release);
    if let Err((_, why)) = SLOT.send(Msg::Begin(id), |tx, rx| start(app, tx, rx)) {
        crate::status::report(app, &format!("Cue could not start its speech engine: {why}"));
    }
    id
}

/// Audio for dictation `id`, 16 kHz mono. Called from the audio thread every 200 ms while recording.
pub fn feed(id: u64, samples: Vec<f32>) {
    let _ = SLOT.try_send(Msg::Audio(id, samples));
}

/// The user stopped. `done` runs on the speech thread with the whole text or a message the user can act on, exactly once, and not at all for
/// cancelled work. When the speech thread is gone it runs here with an error.
pub fn finish(id: u64, done: impl FnOnce(Result<String, String>) + Send + 'static) {
    let _ = OPEN.compare_exchange(id, 0, Ordering::AcqRel, Ordering::Acquire);
    if let Err(Msg::End(_, done)) = SLOT.try_send(Msg::End(id, Box::new(done))) {
        done(Err(LOST.into()));
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

/// Returns once the worker process has exited, so its files can be deleted.
pub fn unload() {
    OPEN.store(0, Ordering::Release);
    let (ack, done) = mpsc::channel();
    if SLOT.try_send(Msg::Unload(ack)).is_ok() {
        let _ = done.recv_timeout(Duration::from_secs(5));
    }
}

#[cfg(test)]
mod session_tests;
#[cfg(test)]
mod tests;
