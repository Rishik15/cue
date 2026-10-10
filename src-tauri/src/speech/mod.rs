//! Purpose: Speech to text. The recognizer lives in a separate worker process (same exe, `--speech-worker`) that is started when dictation
//! begins and killed after the idle time, so a loaded model costs memory only while it is useful and an engine crash never reaches the app.
//! Contents: on_partial — hear sentences as they are recognised; has_model — is anything installed; begin — start loading on hotkey down; feed / finish — stream the recording and collect its text; cancel — drop work in flight; unload — stop the worker;
//! models — Models page commands. `supervisor` owns the dictation state, `process` the child process, `protocol` the pipe format, `catalog` / `download` the model files.

mod catalog;
mod download;
pub mod models;
mod process;
mod protocol;
mod supervisor;
pub mod worker;

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::OnceLock;
use std::time::Duration;

use tauri::AppHandle;

use supervisor::{Msg, Supervisor};

pub use catalog::DEFAULT as DEFAULT_MODEL;

/// Receives each sentence as it is recognised: dictation id, sentence number, text. Runs on the speech thread.
type PartialSink = Box<dyn Fn(u64, usize, &str) + Send + Sync>;
static PARTIAL: OnceLock<PartialSink> = OnceLock::new();

static TX: OnceLock<Option<Sender<Msg>>> = OnceLock::new();
static NEXT_JOB: AtomicU64 = AtomicU64::new(1);
/// Utterances handed in and not yet answered; lets `cancel` cost one atomic load when nothing is happening.
static PENDING: AtomicUsize = AtomicUsize::new(0);

/// The supervisor thread starts with the first dictation, so a Cue that is never used for speech has no extra thread.
fn start(app: &AppHandle) -> std::io::Result<Sender<Msg>> {
    let (tx, rx) = mpsc::channel();
    let sup = Supervisor::new(app.clone(), tx.clone());
    std::thread::Builder::new().name("speech".into()).stack_size(512 * 1024).spawn(move || supervisor::run(sup, rx))?;
    Ok(tx)
}

fn sender(app: &AppHandle) -> Option<&'static Sender<Msg>> {
    TX.get_or_init(|| start(app).ok()).as_ref()
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
    catalog::MODELS.iter().any(|m| catalog::installed(app, m))
}

/// Hotkey went down: the worker starts loading now, while the user is still speaking. Returns the id of this dictation.
pub fn begin(app: &AppHandle) -> u64 {
    let id = NEXT_JOB.fetch_add(1, Ordering::Relaxed);
    if let Some(tx) = sender(app) {
        PENDING.fetch_add(1, Ordering::AcqRel);
        let _ = tx.send(Msg::Begin(id));
    }
    id
}

/// Audio for dictation `id`, 16 kHz mono. Called from the audio thread every 200 ms while recording.
pub fn feed(id: u64, samples: Vec<f32>) {
    if let Some(tx) = TX.get().and_then(|t| t.as_ref()) {
        let _ = tx.send(Msg::Audio(id, samples));
    }
}

/// The user stopped. `done` runs on the speech thread with the whole text or a message the user can act on, and not at all for
/// cancelled work.
pub fn finish(id: u64, done: impl FnOnce(Result<String, String>) + Send + 'static) {
    let Some(tx) = TX.get().and_then(|t| t.as_ref()) else { return done(Err("Cue could not start its speech engine.".into())) };
    let done = Box::new(move |result| {
        let _ = PENDING.try_update(Ordering::AcqRel, Ordering::Acquire, |n| Some(n.saturating_sub(1))); // a cancel may have zeroed it already
        done(result);
    });
    let _ = tx.send(Msg::End(id, done));
}

/// Escape pressed: nothing in flight is inserted afterwards. Returns whether there was anything to cancel.
pub fn cancel() -> bool {
    if PENDING.swap(0, Ordering::AcqRel) == 0 {
        return false;
    }
    if let Some(tx) = TX.get().and_then(|t| t.as_ref()) {
        let _ = tx.send(Msg::Cancel);
    }
    true
}

/// Returns once the worker process has exited, so its files can be deleted.
pub fn unload() {
    let Some(tx) = TX.get().and_then(|t| t.as_ref()) else { return };
    PENDING.store(0, Ordering::Release);
    let (ack, done) = mpsc::channel();
    if tx.send(Msg::Unload(ack)).is_ok() {
        let _ = done.recv_timeout(Duration::from_secs(5));
    }
}
