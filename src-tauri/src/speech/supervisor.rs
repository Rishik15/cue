//! Purpose: Owns the speech worker process and the dictation in flight. Starts the worker on demand, streams the recording to it while the
//! user speaks, unloads it after the idle time, and recovers from a crash or hang without losing the recording. One thread that blocks
//! on a channel with a deadline: no polling, nothing runs while Cue is idle.
//! Contents: Msg / Done — what other threads send; Supervisor — the state machine; run — the thread loop.

use std::collections::VecDeque;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use tauri::AppHandle;

use super::catalog;
use super::process::{self, Worker};
use super::protocol::{Reply, Request};
use crate::prefs::Prefs;

/// How long a worker may take to load a model (a cold disk can be slow), and the least it gets to answer after the user stops.
const LOAD_LIMIT: Duration = Duration::from_secs(180);
const RUN_FLOOR: Duration = Duration::from_secs(30);
/// After loading with nobody using it yet (the user is still talking) the worker stays at least this long, whatever the unload setting.
const GRACE_SECS: u64 = 90;
const NO_MODEL: &str = "No speech model installed. Open Settings, Models, and download one.";

pub type Done = Box<dyn FnOnce(Result<String, String>) + Send>;

pub enum Msg {
    /// A dictation starts: get the worker loading.
    Begin(u64),
    /// 16 kHz audio for dictation `id`.
    Audio(u64, Vec<f32>),
    /// The user stopped; `done` gets the whole text.
    End(u64, Done),
    Cancel,
    /// Kill the worker and answer once it is gone (the caller is about to delete its files).
    Unload(Sender<()>),
    Reply(u64, Reply),
    Gone(u64),
}

/// A finished recording waiting for its text. The audio is kept until then so a crash can replay it on a fresh worker.
struct Job {
    id: u64,
    audio: Vec<f32>,
    done: Done,
}

/// The dictation being recorded right now.
struct Session {
    id: u64,
    audio: Vec<f32>,
    /// The worker could not start; reported when the dictation ends.
    failed: Option<String>,
}

#[derive(PartialEq, Clone, Copy)]
enum Why {
    Idle,
    Load,
    Run,
}

pub struct Supervisor {
    app: AppHandle,
    tx: Sender<Msg>,
    worker: Option<Worker>,
    serials: u64,
    /// Requests waiting for the model to finish loading.
    backlog: Vec<(Request, Vec<f32>)>,
    session: Option<Session>,
    running: VecDeque<Job>,
    retried: bool,
    deadline: Option<(Instant, Why)>,
}

impl Supervisor {
    pub fn new(app: AppHandle, tx: Sender<Msg>) -> Self {
        Supervisor {
            app,
            tx,
            worker: None,
            serials: 0,
            backlog: Vec::new(),
            session: None,
            running: VecDeque::new(),
            retried: false,
            deadline: None,
        }
    }

    fn kill(&mut self) {
        if let Some(w) = self.worker.take() {
            w.kill();
            crate::mem::trim_soon();
        }
        self.backlog.clear();
        self.deadline = None;
    }

    /// Starts a worker and queues its model load; everything sent before the load finishes waits in `backlog`.
    fn spawn(&mut self) -> Result<(), String> {
        let prefs = Prefs::load(&self.app);
        let installed = |m: &&catalog::Model| catalog::installed(&self.app, m);
        // The chosen model, or any installed one, so dictation keeps working after the chosen one was deleted.
        let model =
            catalog::find(&prefs.speech_model).filter(installed).or_else(|| catalog::MODELS.iter().find(installed)).ok_or(NO_MODEL)?;
        let path = |p: Option<std::path::PathBuf>| p.map(|p| p.to_string_lossy().replace('\\', "/")).ok_or(NO_MODEL);
        let (dir, vad) = (path(catalog::dir(&self.app, model))?, path(catalog::vad_path(&self.app))?);
        self.serials += 1;
        let (serial, tx) = (self.serials, self.tx.clone());
        let worker = process::spawn(serial, move |reply| {
            let _ = tx.send(reply.map_or(Msg::Gone(serial), |r| Msg::Reply(serial, r)));
        })?;
        worker.send(Request::Load { dir, vad, family: model.family, threads: process::threads() }, Vec::new());
        self.worker = Some(worker);
        self.deadline = Some((Instant::now() + LOAD_LIMIT, Why::Load));
        Ok(())
    }

    fn send(&mut self, request: Request, samples: Vec<f32>) {
        match self.worker.as_ref().filter(|w| w.ready) {
            Some(w) => w.send(request, samples),
            None => self.backlog.push((request, samples)),
        }
    }

    pub fn begin(&mut self, id: u64) {
        let mut failed = None;
        if self.worker.is_none() {
            failed = self.spawn().err();
        }
        if let Some(problem) = &failed {
            crate::status::report(&self.app, problem);
        }
        self.session = Some(Session { id, audio: Vec::new(), failed });
        self.send(Request::Begin { id }, Vec::new());
        self.settle(GRACE_SECS);
    }

    pub fn audio(&mut self, id: u64, samples: Vec<f32>) {
        let Some(s) = self.session.as_mut().filter(|s| s.id == id && s.failed.is_none()) else { return };
        s.audio.extend_from_slice(&samples);
        self.send(Request::Audio, samples);
    }

    pub fn end(&mut self, id: u64, done: Done) {
        let Some(session) = self.session.take().filter(|s| s.id == id) else { return }; // cancelled meanwhile
        if let Some(problem) = session.failed {
            return done(Err(problem));
        }
        self.send(Request::End { id }, Vec::new());
        self.running.push_back(Job { id, audio: session.audio, done });
        self.settle(0);
    }

    pub fn reply(&mut self, serial: u64, reply: Reply) {
        if self.worker.as_ref().map(|w| w.serial) != Some(serial) {
            return; // from a worker we already killed
        }
        match reply {
            Reply::Loaded { .. } => self.loaded(),
            Reply::Text { id, text } => {
                if let Some(job) = self.running.pop_front_if(|j| j.id == id) {
                    self.retried = false;
                    (job.done)(Ok(text));
                }
                self.settle(0);
            }
            Reply::Partial { id, index, text } => super::emit_partial(id, index, &text),
            Reply::Error { message } => self.fail(&message),
        }
    }

    fn loaded(&mut self) {
        let Some(w) = self.worker.as_mut() else { return };
        w.ready = true;
        for (request, samples) in std::mem::take(&mut self.backlog) {
            w.send(request, samples);
        }
        self.settle(GRACE_SECS);
    }

    /// Decides what the deadline is now: answering a finished recording, nothing while the user is still talking, or the idle unload
    /// (at least `min_secs`; the user's "Immediately" unloads at once only when `min_secs` is 0).
    fn settle(&mut self, min_secs: u64) {
        if !self.worker.as_ref().is_some_and(|w| w.ready) {
            return;
        }
        if !self.running.is_empty() {
            self.deadline = Some((Instant::now() + RUN_FLOOR, Why::Run));
        } else if self.session.is_some() {
            self.deadline = None;
        } else {
            // Seconds the engine stays loaded after use: -1 never, 0 right away.
            match crate::settings::get_i64(&self.app, "unload_after_secs", 120) {
                s if s < 0 => self.deadline = None,
                0 if min_secs == 0 => self.kill(),
                s => self.deadline = Some((Instant::now() + Duration::from_secs((s as u64).max(min_secs)), Why::Idle)),
            }
        }
    }

    /// Stops everything and tells whoever is waiting.
    fn fail(&mut self, message: &str) {
        self.kill();
        self.retried = false;
        if let Some(s) = self.session.as_mut() {
            s.failed = Some(message.to_string());
        }
        for job in self.running.drain(..) {
            (job.done)(Err(message.to_string()));
        }
    }

    pub fn gone(&mut self, serial: u64) {
        if self.worker.as_ref().map(|w| w.serial) == Some(serial) {
            self.recover();
        }
    }

    /// The worker died. The recordings it held are replayed on one fresh worker; a second death in a row is reported.
    fn recover(&mut self) {
        self.kill();
        if self.running.is_empty() && self.session.is_none() {
            return;
        }
        if self.retried {
            return self.fail("The speech engine stopped unexpectedly. Try dictating again.");
        }
        if let Err(problem) = self.spawn() {
            return self.fail(&problem);
        }
        self.retried = true;
        for i in 0..self.running.len() {
            let (id, audio) = (self.running[i].id, self.running[i].audio.clone());
            self.backlog.extend([(Request::Begin { id }, Vec::new()), (Request::Audio, audio), (Request::End { id }, Vec::new())]);
        }
        if let Some(s) = &self.session {
            self.backlog.extend([(Request::Begin { id: s.id }, Vec::new()), (Request::Audio, s.audio.clone())]);
        }
    }

    /// Escape: drop the recording in progress and kill the worker if it is busy with a finished one (no late text is ever inserted).
    pub fn cancel(&mut self) {
        self.session = None;
        self.backlog.clear();
        if !self.running.is_empty() {
            self.running.clear();
            self.kill();
        }
    }

    pub fn unload(&mut self) {
        self.session = None;
        self.running.clear();
        self.kill();
    }

    fn expired(&mut self, why: Why) {
        match why {
            Why::Idle => self.kill(),
            Why::Load => self.fail("The speech model took too long to load."),
            Why::Run => self.fail("Transcription took too long and was stopped."),
        }
    }
}

pub fn run(mut sup: Supervisor, rx: Receiver<Msg>) {
    loop {
        // Blocks until a message, or until the load / run / idle deadline. No polling.
        let msg = match sup.deadline {
            Some((at, why)) => match rx.recv_timeout(at.saturating_duration_since(Instant::now())) {
                Err(RecvTimeoutError::Timeout) => {
                    sup.expired(why);
                    continue;
                }
                Err(RecvTimeoutError::Disconnected) => return,
                Ok(m) => m,
            },
            None => match rx.recv() {
                Ok(m) => m,
                Err(_) => return,
            },
        };
        match msg {
            Msg::Begin(id) => sup.begin(id),
            Msg::Audio(id, samples) => sup.audio(id, samples),
            Msg::End(id, done) => sup.end(id, done),
            Msg::Cancel => sup.cancel(),
            Msg::Unload(ack) => {
                sup.unload();
                let _ = ack.send(());
            }
            Msg::Reply(serial, reply) => sup.reply(serial, reply),
            Msg::Gone(serial) => sup.gone(serial),
        }
    }
}
