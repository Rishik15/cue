//! File: speech/session.rs
//! Purpose: Every rule of a dictation session as a pure state machine: begin, audio, end, cancel, unload, worker replies, a worker that died,
//! the deadlines, the one retry and the replay. It never touches a process, a clock or a setting: it is fed events with the time and answers
//! with effects, so every rule is a table test with a fake clock and a scripted worker. `pump` is the thin shell that runs the effects.
//! Contents: Pick — the model choice; Event / Effect — what goes in and out; Session — the machine (step, deadline, quiescent). The worker
//! speaks one dictation at a time, so Begin B waits on the wire until End A has gone out (`advance`); late audio finds its dictation by id.

use std::time::{Duration, Instant};

use super::protocol::{EngineSpec, Reply, Request, VERSION};
use super::take::{Done, Take};

/// How long a worker may take to load a model (a cold disk can be slow).
const LOAD_LIMIT: Duration = Duration::from_secs(180);
/// The least a worker gets to answer after the user stops (longer for a long recording: at least real time, so even the slowest model is never cut off).
const RUN_FLOOR: Duration = Duration::from_secs(30);
/// How long a worker may stay silent while it has requests to answer and nobody waits for text yet: it answers every request, so this is a hang.
const STALL_LIMIT: Duration = Duration::from_secs(120);
/// After loading with nobody using it yet (the user is still talking) the worker stays at least this long, whatever the unload setting.
const GRACE_SECS: u64 = 90;
/// A recording that was superseded by a newer one and has heard nothing for this long was lost by its owner: it is dropped.
const ABANDON_AFTER: Duration = Duration::from_secs(20);
/// A recording still open this long after its cap should have ended it is stuck: it is dropped so it cannot hold the model forever.
const CAP_MARGIN: Duration = Duration::from_secs(30);
/// How many recordings without their End are remembered; the oldest is dropped past this.
const OPEN_MAX: usize = 4;

pub const STOPPED: &str = "The speech engine stopped before this recording finished.";
pub const NO_MODEL: &str = "No speech model installed. Open Settings, Models, and download one.";
const CRASHED: &str = "The speech engine stopped unexpectedly. Try dictating again.";
const LOAD_SLOW: &str = "The speech model took too long to load.";
const RUN_SLOW: &str = "Transcription took too long and was stopped.";
const HUNG: &str = "The speech engine stopped responding.";
const OVERLOADED: &str = "The speech engine fell too far behind the recording.";
const MISMATCH: &str = "The speech engine is out of date. Restart Cue.";
const LOST: &str = "Cue lost track of this recording. Try dictating again.";
const DROPPED: &str = "A recording ran too long without ending and was dropped.";

/// The model a worker loads: its catalog id and how the worker wires its files.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pick {
    pub id: &'static str,
    pub spec: &'static EngineSpec,
}

pub enum Event {
    /// A dictation starts. `model` is the user's choice now (or why there is none); `cap` the longest recording they allow.
    Begin {
        id: u64,
        model: Result<Pick, String>,
        cap: Duration,
    },
    /// 16 kHz audio for dictation `id`, possibly after the next dictation began.
    Audio {
        id: u64,
        samples: Vec<f32>,
    },
    /// The user stopped; `done` gets the whole text.
    End {
        id: u64,
        done: Done,
    },
    /// Escape: drop the dictation `id` if it is still being recorded.
    Cancel {
        id: u64,
    },
    /// Kill the worker: the caller is about to delete its files.
    Unload,
    Reply {
        serial: u64,
        reply: Reply,
    },
    Gone {
        serial: u64,
    },
    SpawnFailed {
        serial: u64,
        message: String,
    },
    /// The worker's input queue is full of audio it has not read.
    Overload {
        serial: u64,
    },
    /// The time passed a deadline.
    Tick,
}

pub enum Effect {
    Spawn {
        serial: u64,
        pick: Pick,
    },
    Send {
        serial: u64,
        request: Request,
        samples: Vec<f32>,
    },
    Kill {
        serial: u64,
    },
    Deliver(Done, Result<String, String>),
    Report(String),
    Partial {
        id: u64,
        index: usize,
        text: String,
    },
    Trim,
    /// A line for cue.log: timings per model, never what was said.
    Log(String),
}

#[derive(Clone, Copy)]
enum Why {
    Load,
    Run,
    Stall,
    Idle,
    Lost(u64, bool),
}

struct Live {
    serial: u64,
    ready: bool,
    pick: Pick,
}

/// One dictation. Recording while `done` is None; once ended it waits (on the wire or behind an earlier dictation) for its text.
struct Entry {
    take: Take,
    done: Option<Done>,
    failed: Option<String>,
    begun: bool,
    stopped: Option<Instant>,
    seen: Instant,
    cap_at: Instant,
}

pub struct Session {
    worker: Option<Live>,
    serials: u64,
    backlog: Vec<(Request, Vec<f32>)>,
    queue: Vec<Entry>,
    retried: bool,
    wire: Option<u64>,
    pending: usize,
    progress: Instant,
    load_by: Option<Instant>,
    idle_at: Option<Instant>,
    unload_secs: i64,
    now: Instant,
    out: Vec<Effect>,
}

impl Session {
    /// A machine with no worker and no dictation. Input: the time it starts at.
    pub fn new(now: Instant) -> Self {
        Session {
            worker: None,
            serials: 0,
            backlog: Vec::new(),
            queue: Vec::new(),
            retried: false,
            wire: None,
            pending: 0,
            progress: now,
            load_by: None,
            idle_at: None,
            unload_secs: 120,
            now,
            out: Vec::new(),
        }
    }

    /// Seconds the engine stays loaded after use (the user's setting): -1 never, 0 right away. Input: the setting as read just now.
    pub fn set_unload_after(&mut self, secs: i64) {
        self.unload_secs = secs;
    }

    /// Applies one event at time `now`. Returns the effects to run, in order.
    pub fn step(&mut self, event: Event, now: Instant) -> Vec<Effect> {
        self.now = now;
        match event {
            Event::Begin { id, model, cap } => self.begin(id, model, cap),
            Event::Audio { id, samples } => self.audio(id, samples),
            Event::End { id, done } => self.end(id, done),
            Event::Cancel { id } => self.cancel(id),
            Event::Unload => self.unload(),
            Event::Reply { serial, reply } if self.is_current(serial) => self.reply(reply),
            Event::Gone { serial } if self.is_current(serial) => self.recover(),
            Event::SpawnFailed { serial, message } if self.is_current(serial) => {
                self.out.push(Effect::Report(message.clone()));
                self.fail(&message);
            }
            Event::Overload { serial } if self.is_current(serial) => self.fail(OVERLOADED),
            Event::Tick => self.expire(),
            _ => {}
        }
        std::mem::take(&mut self.out)
    }

    /// When the next deadline falls, or None when nothing is waiting on the clock (the thread then blocks for good).
    pub fn deadline(&self) -> Option<Instant> {
        self.due().map(|(at, _)| at)
    }

    /// Nothing is loaded and nothing is in flight: the speech thread may leave.
    pub fn quiescent(&self) -> bool {
        self.worker.is_none() && self.queue.is_empty()
    }

    fn is_current(&self, serial: u64) -> bool {
        self.worker.as_ref().is_some_and(|w| w.serial == serial)
    }

    fn begin(&mut self, id: u64, model: Result<Pick, String>, cap: Duration) {
        self.retried = false;
        let failed = match self.choose(model) {
            Ok(pick) => {
                if self.worker.is_none() {
                    self.spawn(pick);
                }
                None
            }
            Err(problem) => {
                self.out.push(Effect::Report(problem.clone()));
                Some(problem)
            }
        };
        self.drop_surplus();
        let entry = Entry {
            take: Take::new(id),
            done: None,
            failed,
            begun: false,
            stopped: None,
            seen: self.now,
            cap_at: self.now + cap + CAP_MARGIN,
        };
        self.queue.push(entry);
        self.advance();
        self.settle(GRACE_SECS);
    }

    /// The model the new dictation uses: the user's choice, unless dictations are still in flight, which keep the worker they were started on
    /// (a switch waits for them instead of killing their audio). A changed choice with nothing in flight replaces the worker.
    fn choose(&mut self, model: Result<Pick, String>) -> Result<Pick, String> {
        let Some(live) = self.worker.as_ref() else { return model };
        if !self.queue.is_empty() {
            return Ok(live.pick);
        }
        if !model.as_ref().is_ok_and(|p| p.id == live.pick.id) {
            self.kill();
        }
        model
    }

    /// Past `OPEN_MAX` recordings without an End the oldest is lost for good.
    fn drop_surplus(&mut self) {
        while self.queue.iter().filter(|e| e.done.is_none()).count() >= OPEN_MAX {
            let Some(at) = self.queue.iter().position(|e| e.done.is_none()) else { return };
            self.remove_open(at);
        }
    }

    fn spawn(&mut self, pick: Pick) {
        self.serials += 1;
        self.worker = Some(Live { serial: self.serials, ready: false, pick });
        self.load_by = Some(self.now + LOAD_LIMIT);
        self.out.push(Effect::Spawn { serial: self.serials, pick });
    }

    fn kill(&mut self) {
        if let Some(w) = self.worker.take() {
            self.out.push(Effect::Kill { serial: w.serial });
            self.out.push(Effect::Trim);
        }
        (self.wire, self.load_by, self.idle_at, self.pending) = (None, None, None, 0);
        self.backlog.clear();
    }

    /// Queues a request for the worker: straight out when it is loaded, otherwise after the load.
    fn send(&mut self, request: Request, samples: Vec<f32>) {
        match self.worker.as_ref().filter(|w| w.ready).map(|w| w.serial) {
            Some(serial) => self.emit(serial, request, samples),
            None => self.backlog.push((request, samples)),
        }
    }

    fn emit(&mut self, serial: u64, request: Request, samples: Vec<f32>) {
        if self.pending == 0 {
            self.progress = self.now;
        }
        self.pending += 1;
        self.out.push(Effect::Send { serial, request, samples });
    }

    /// Puts the next waiting dictations on the wire in the order they began. The worker holds one dictation at a time, so a dictation that
    /// began while an earlier one is still open waits here, with its audio kept, until the earlier End has gone out; one that already ended
    /// goes out whole (Begin, audio, End). Also how a fresh worker is given everything after a crash.
    fn advance(&mut self) {
        while self.wire.is_none() {
            let Some(at) = self.queue.iter().position(|e| !e.begun) else { return };
            let entry = &mut self.queue[at];
            entry.begun = true;
            if entry.failed.is_some() {
                continue;
            }
            let finished = entry.done.is_some();
            let requests = entry.take.replay(finished);
            if !finished {
                self.wire = Some(entry.take.id);
            }
            for (request, samples) in requests {
                self.send(request, samples);
            }
        }
    }

    fn audio(&mut self, id: u64, samples: Vec<f32>) {
        let Some(entry) = self.queue.iter_mut().find(|e| e.take.id == id && e.done.is_none() && e.failed.is_none()) else { return };
        entry.seen = self.now;
        entry.take.push(&samples);
        if entry.begun {
            self.send(Request::Audio, samples);
        }
    }

    fn end(&mut self, id: u64, done: Done) {
        let Some(at) = self.queue.iter().position(|e| e.take.id == id && e.done.is_none()) else {
            return self.out.push(Effect::Deliver(done, Err(LOST.into())));
        };
        let problem = self.queue[at].failed.clone().or_else(|| self.worker.is_none().then(|| STOPPED.to_string()));
        if let Some(problem) = problem {
            let entry = self.queue.remove(at);
            self.deliver(done, &entry.take, &problem);
        } else {
            self.queue[at].done = Some(done);
            self.queue[at].stopped = Some(self.now);
            if self.wire == Some(id) {
                self.wire = None;
                self.send(Request::End { id }, Vec::new());
            }
        }
        self.advance();
        self.settle(0);
    }

    fn cancel(&mut self, id: u64) {
        if let Some(at) = self.queue.iter().position(|e| e.take.id == id && e.done.is_none()) {
            self.remove_open(at);
        }
    }

    /// Forgets the recording at `at` (cancelled, lost or surplus): nothing of it may reach a worker that is still loading, and the dictations
    /// waiting behind it go out.
    fn remove_open(&mut self, at: usize) {
        let id = self.queue.remove(at).take.id;
        if self.wire == Some(id) {
            self.wire = None;
            if let Some(at) = self.backlog.iter().position(|(r, _)| matches!(r, Request::Begin { id: begun, .. } if *begun == id)) {
                self.backlog.truncate(at);
            }
        }
        self.advance();
        self.settle(0);
    }

    fn unload(&mut self) {
        self.kill();
        self.retried = false;
        for entry in std::mem::take(&mut self.queue) {
            if let Some(done) = entry.done {
                self.deliver(done, &entry.take, STOPPED);
            }
        }
    }

    fn reply(&mut self, reply: Reply) {
        self.progress = self.now;
        match reply {
            Reply::Loaded { version, .. } if version != VERSION => self.fail(MISMATCH),
            Reply::Loaded { ms, .. } => self.loaded(ms),
            Reply::Release { id, up_to } => {
                self.pending = self.pending.saturating_sub(1);
                if let Some(entry) = self.queue.iter_mut().find(|e| e.take.id == id) {
                    entry.take.release(up_to);
                }
            }
            Reply::Partial { id, index, text } => self.partial(id, index, text),
            Reply::Text { id, .. } => self.text(id),
            Reply::Error { message } => self.fail(&message),
        }
    }

    fn loaded(&mut self, ms: u64) {
        let Some(w) = self.worker.as_mut() else { return };
        w.ready = true;
        let (serial, pick) = (w.serial, w.pick);
        self.out.push(Effect::Log(format!("speech: {} ({:?}) loaded in {ms} ms", pick.id, pick.spec.kind)));
        self.load_by = None;
        for (request, samples) in std::mem::take(&mut self.backlog) {
            self.emit(serial, request, samples);
        }
        self.settle(GRACE_SECS);
    }

    /// A clause was recognised: keep it and show it.
    fn partial(&mut self, id: u64, index: usize, text: String) {
        let Some(entry) = self.queue.iter_mut().find(|e| e.take.id == id) else { return };
        if let Some(index) = entry.take.commit(index, text.clone()) {
            self.out.push(Effect::Partial { id, index, text });
        }
    }

    /// The worker finished a dictation: the app's own clauses are the text (the worker's own only covers what it heard since it started).
    fn text(&mut self, id: u64) {
        if let Some(at) = self.queue.iter().position(|e| e.take.id == id && e.begun && e.done.is_some()) {
            let entry = self.queue.remove(at);
            self.retried = false;
            let model = self.worker.as_ref().map_or("?", |w| w.pick.id);
            let waited = entry.stopped.map_or(0, |at| self.now.saturating_duration_since(at).as_millis());
            self.out.push(Effect::Log(format!("speech: {model} text {waited} ms after stop")));
            if let Some(done) = entry.done {
                self.out.push(Effect::Deliver(done, Ok(entry.take.text())));
            }
        }
        self.settle(0);
    }

    /// Decides the idle clock: nothing to unload while any dictation is in flight; otherwise the user's unload setting (at least `min_secs`;
    /// "Immediately" unloads at once only when `min_secs` is 0).
    fn settle(&mut self, min_secs: u64) {
        if !self.worker.as_ref().is_some_and(|w| w.ready) {
            return;
        }
        self.idle_at = None;
        if !self.queue.is_empty() {
            return;
        }
        match self.unload_secs {
            s if s < 0 => {}
            0 if min_secs == 0 => self.kill(),
            s => self.idle_at = Some(self.now + Duration::from_secs((s as u64).max(min_secs))),
        }
    }

    /// Stops everything and tells whoever is waiting: finished dictations get what was recognised, recordings in progress fail when they end.
    fn fail(&mut self, message: &str) {
        self.kill();
        self.retried = false;
        for mut entry in std::mem::take(&mut self.queue) {
            match entry.done.take() {
                Some(done) => self.deliver(done, &entry.take, message),
                None => {
                    (entry.failed, entry.begun) = (Some(message.to_string()), true);
                    self.queue.push(entry);
                }
            }
        }
    }

    /// The engine stopped, but whatever it had recognised before is still the user's words: hand it over (and say what happened) instead of
    /// dropping it. Only a dictation with nothing recognised is a failure.
    fn deliver(&mut self, done: Done, take: &Take, message: &str) {
        let kept = take.text();
        if kept.is_empty() {
            return self.out.push(Effect::Deliver(done, Err(message.to_string())));
        }
        self.out.push(Effect::Report(format!("{message} What was recognised so far was kept.")));
        self.out.push(Effect::Deliver(done, Ok(kept)));
    }

    /// The worker died. Every dictation it held is replayed on one fresh worker; a second death in a row is reported.
    fn recover(&mut self) {
        let pick = self.worker.as_ref().map(|w| w.pick);
        self.kill();
        if self.queue.is_empty() {
            return;
        }
        if self.retried {
            return self.fail(CRASHED);
        }
        let Some(pick) = pick else { return self.fail(NO_MODEL) };
        self.spawn(pick);
        self.retried = true;
        for entry in &mut self.queue {
            entry.begun = entry.failed.is_some();
        }
        self.advance();
    }

    /// The earliest deadline and what it is for.
    fn due(&self) -> Option<(Instant, Why)> {
        let timers = [self.load_by.map(|at| (at, Why::Load)), self.busy(), self.idle_at.map(|at| (at, Why::Idle))];
        timers.into_iter().flatten().chain(self.lost()).min_by_key(|(at, _)| *at)
    }

    /// The worker has requests to answer: it must say something within the limit, which counts from its last word. After End the limit is at
    /// least the length of the audio still to recognise.
    fn busy(&self) -> Option<(Instant, Why)> {
        if self.pending == 0 || !self.worker.as_ref().is_some_and(|w| w.ready) {
            return None;
        }
        match self.queue.iter().find(|e| e.begun && e.done.is_some()) {
            Some(e) => Some((self.progress + RUN_FLOOR.max(Duration::from_secs_f32(e.take.seconds())), Why::Run)),
            None => Some((self.progress + STALL_LIMIT, Why::Stall)),
        }
    }

    /// When a recording that never ended is given up on: after the cap, or soon after a newer dictation took over and it went quiet.
    fn lost(&self) -> Option<(Instant, Why)> {
        let last = self.queue.len().saturating_sub(1);
        let open = self.queue.iter().enumerate().filter(|(_, e)| e.done.is_none());
        let due =
            open.map(|(i, e)| (if i < last { e.cap_at.min(e.seen + ABANDON_AFTER) } else { e.cap_at }, Why::Lost(e.take.id, i < last)));
        due.min_by_key(|(at, _)| *at)
    }

    /// Acts on every deadline that has passed.
    fn expire(&mut self) {
        while let Some((at, why)) = self.due() {
            if at > self.now {
                return;
            }
            match why {
                Why::Load => self.fail(LOAD_SLOW),
                Why::Run => self.fail(RUN_SLOW),
                Why::Stall => self.fail(HUNG),
                Why::Idle => self.kill(),
                Why::Lost(id, superseded) => self.give_up(id, superseded),
            }
        }
    }

    /// Drops a recording nobody ended. Only a recording that outlived its cap is worth telling the user about.
    fn give_up(&mut self, id: u64, superseded: bool) {
        if !superseded {
            self.out.push(Effect::Report(DROPPED.into()));
        }
        if let Some(at) = self.queue.iter().position(|e| e.take.id == id) {
            self.remove_open(at);
        }
    }
}
