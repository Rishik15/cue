//! Purpose: Owns the speech worker process and the dictation in flight. Starts the worker on demand, streams the recording to it while the
//! user speaks, unloads it after the idle time, and recovers from a crash or hang without losing the recording. One thread that blocks
//! on a channel with a deadline: no polling, nothing runs while Cue is idle.
//! Contents: Supervisor — the state machine (begin, audio, end, cancel, replies, recovery); the messages and the thread loop are in `pump`.

use std::collections::VecDeque;
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use tauri::AppHandle;

use super::catalog;
use super::process::{self, Worker};
use super::protocol::{Reply, Request};
use super::pump::Msg;
use super::take::{self, Done, Job, Session, Take};

/// How long a worker may take to load a model (a cold disk can be slow), and the least it gets to answer after the user stops
/// (longer for a long recording: at least real time, so even the slowest model is never cut off).
const LOAD_LIMIT: Duration = Duration::from_secs(180);
const RUN_FLOOR: Duration = Duration::from_secs(30);
/// After loading with nobody using it yet (the user is still talking) the worker stays at least this long, whatever the unload setting.
const GRACE_SECS: u64 = 90;
/// How many stopped-but-not-yet-ended recordings are remembered.
const PARKED_MAX: usize = 4;
const STOPPED: &str = "The speech engine stopped before this recording finished.";
const NO_MODEL: &str = "No speech model installed. Open Settings, Models, and download one.";

#[derive(Clone, Copy)]
pub(super) enum Why {
    Idle,
    Load,
    Run,
}

pub struct Supervisor {
    app: AppHandle,
    tx: Sender<Msg>,
    worker: Option<Worker>,
    /// The model the worker was started with.
    model: Option<&'static catalog::Model>,
    serials: u64,
    /// Requests waiting for the model to finish loading.
    backlog: Vec<(Request, Vec<f32>)>,
    session: Option<Session>,
    /// Recordings that were stopped but whose `end` has not arrived yet because the next dictation already began: kept so their text is still delivered.
    parked: Vec<Session>,
    running: VecDeque<Job>,
    retried: bool,
    pub(super) deadline: Option<(Instant, Why)>,
}

impl Supervisor {
    pub fn new(app: AppHandle, tx: Sender<Msg>) -> Self {
        Supervisor {
            app,
            tx,
            worker: None,
            model: None,
            serials: 0,
            backlog: Vec::new(),
            session: None,
            parked: Vec::new(),
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
        (self.model, self.deadline) = (None, None);
        self.backlog.clear();
    }

    /// Starts a worker and queues its model load; everything sent before the load finishes waits in `backlog`.
    fn spawn(&mut self, model: &'static catalog::Model) -> Result<(), String> {
        let path = |p: Option<std::path::PathBuf>| p.map(|p| p.to_string_lossy().replace('\\', "/")).ok_or(NO_MODEL);
        let (dir, vad) = (path(catalog::dir(&self.app, model))?, path(catalog::vad_path(&self.app))?);
        self.serials += 1;
        let (serial, tx) = (self.serials, self.tx.clone());
        let worker = process::spawn(serial, move |reply| {
            let _ = tx.send(reply.map_or(Msg::Gone(serial), |r| Msg::Reply(serial, r)));
        })?;
        worker.send(Request::Load { dir, vad, family: model.family, threads: process::threads() }, Vec::new());
        (self.worker, self.model) = (Some(worker), Some(model));
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
        // Each dictation gets its own one retry; a stale flag would fail a crash days later.
        self.retried = false;
        // A dictation still being recognised keeps its worker; otherwise a changed model choice takes effect now.
        let want = if self.running.is_empty() { catalog::pick(&self.app) } else { self.model };
        if self.running.is_empty() && self.model.map(|m| m.id) != want.map(|m| m.id) {
            self.kill();
        }
        let failed = if self.worker.is_none() { want.ok_or_else(|| NO_MODEL.to_string()).and_then(|m| self.spawn(m)).err() } else { None };
        if let Some(problem) = &failed {
            crate::status::report(&self.app, problem);
        }
        if let Some(old) = self.session.take() {
            self.parked.push(old);
            if self.parked.len() > PARKED_MAX {
                self.parked.remove(0);
            }
        }
        self.session = Some(Session { take: Take::new(id), failed });
        self.send(Request::Begin { id }, Vec::new());
        self.settle(GRACE_SECS);
    }

    pub fn audio(&mut self, id: u64, samples: Vec<f32>) {
        let Some(s) = self.session.as_mut().filter(|s| s.take.id == id && s.failed.is_none()) else { return };
        s.take.push(&samples);
        self.send(Request::Audio, samples);
    }

    pub fn end(&mut self, id: u64, done: Done) {
        let current = self.session.take_if(|s| s.take.id == id);
        let Some(session) = current.or_else(|| self.parked.iter().position(|s| s.take.id == id).map(|at| self.parked.remove(at))) else {
            return done(Err("Cue lost track of this recording. Try dictating again.".into()));
        };
        if let Some(problem) = session.failed.or_else(|| self.worker.is_none().then(|| STOPPED.to_string())) {
            return done(self.keep_or_fail(&session.take, &problem));
        }
        self.send(Request::End { id }, Vec::new());
        self.running.push_back(Job { take: session.take, done });
        self.settle(0);
    }

    pub fn reply(&mut self, serial: u64, reply: Reply) {
        if self.worker.as_ref().map(|w| w.serial) != Some(serial) {
            return; // from a worker we already killed
        }
        match reply {
            Reply::Loaded { .. } => self.loaded(),
            Reply::Text { id, .. } => {
                // The worker's own text only covers what it heard since it started; the app's `take` has every sentence.
                if let Some(job) = self.running.pop_front_if(|j| j.take.id == id) {
                    self.retried = false;
                    (job.done)(Ok(job.take.text()));
                }
                self.settle(0);
            }
            Reply::Partial { id, text, end, .. } => self.partial(id, text, end),
            Reply::Error { message } => self.fail(&message),
        }
    }

    /// A sentence was recognised: keep it, let the audio it used up go, and show it.
    fn partial(&mut self, id: u64, text: String, end: usize) {
        let mut takes = self.session.iter_mut().map(|s| &mut s.take).chain(self.running.iter_mut().map(|j| &mut j.take));
        let Some(take) = takes.find(|t| t.id == id) else { return };
        let index = take.commit(text.clone(), end);
        super::emit_partial(id, index, &text);
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
        if let Some(job) = self.running.front() {
            let audio = Duration::from_secs_f32(job.take.seconds());
            self.deadline = Some((Instant::now() + RUN_FLOOR.max(audio), Why::Run));
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
        for s in self.session.iter_mut().chain(self.parked.iter_mut()) {
            s.failed = Some(message.to_string());
        }
        for job in std::mem::take(&mut self.running) {
            (job.done)(self.keep_or_fail(&job.take, message));
        }
    }

    /// The engine stopped, but whatever it had recognised before is still the user's words: hand it over (and say what happened) instead of
    /// dropping it. Only a dictation with nothing recognised is a failure.
    fn keep_or_fail(&self, take: &Take, message: &str) -> Result<String, String> {
        let kept = take.text();
        if kept.is_empty() {
            return Err(message.to_string());
        }
        crate::status::report(&self.app, &format!("{message} What was recognised so far was kept."));
        Ok(kept)
    }

    pub fn gone(&mut self, serial: u64) {
        if self.worker.as_ref().map(|w| w.serial) == Some(serial) {
            self.recover();
        }
    }

    /// The worker died. The recordings it held are replayed on one fresh worker; a second death in a row is reported.
    fn recover(&mut self) {
        let model = self.model;
        self.kill();
        if self.running.is_empty() && self.session.is_none() && self.parked.is_empty() {
            return;
        }
        if self.retried {
            return self.fail("The speech engine stopped unexpectedly. Try dictating again.");
        }
        if let Err(problem) = model.ok_or_else(|| NO_MODEL.to_string()).and_then(|m| self.spawn(m)) {
            return self.fail(&problem);
        }
        self.retried = true;
        self.backlog.extend(take::replay_all(&mut self.running, &mut self.parked, self.session.as_mut()));
    }

    /// Escape: drop the recording in progress. Dictations the user already stopped keep running and are still delivered: spoken text is never thrown away.
    pub fn cancel(&mut self) {
        let Some(cancelled) = self.session.take() else { return };
        let id = cancelled.take.id;
        // Nothing of this recording may reach a worker that is still loading; what earlier recordings queued stays.
        if let Some(at) = self.backlog.iter().position(|(r, _)| matches!(r, Request::Begin { id: begun } if *begun == id)) {
            self.backlog.truncate(at);
        }
    }

    pub fn unload(&mut self) {
        self.session = None;
        self.parked.clear();
        self.kill();
        for job in std::mem::take(&mut self.running) {
            (job.done)(self.keep_or_fail(&job.take, STOPPED));
        }
    }

    pub(super) fn expired(&mut self, why: Why) {
        match why {
            Why::Idle => self.kill(),
            Why::Load => self.fail("The speech model took too long to load."),
            Why::Run => self.fail("Transcription took too long and was stopped."),
        }
    }
}
