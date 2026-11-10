//! File: speech/pump.rs
//! Purpose: The speech thread: the thin shell around `session`. It turns what other threads send into session events, runs the effects the session
//! answers with (start or kill the worker process, write to its pipe, call the dictation's callback, tell the user), and blocks on the channel
//! with the session's deadline. No polling: nothing runs while Cue is idle, and the thread leaves when there is nothing left to do.
//! Contents: Msg — what other threads send; Shell — the session plus the worker process; guarded — one step whose panic is contained and cleaned up after; run — the thread loop.

use std::collections::VecDeque;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use tauri::AppHandle;

use super::catalog;
use super::events::{Effect, Event, Pick, Unload, NO_MODEL};
use super::process::{self, Worker};
use super::protocol::{Reply, Request, VERSION};
use super::session::Session;
use super::take::Done;
use crate::guard;

/// What other threads send the speech thread.
pub enum Msg {
    /// A dictation starts: get the worker loading.
    Begin(u64),
    /// 16 kHz audio for dictation `id`.
    Audio(u64, Vec<f32>),
    /// The user stopped; `done` gets the whole text.
    End(u64, Done),
    Cancel(u64),
    /// Kill the worker and answer once it is gone (the caller is about to delete its files).
    Unload(Sender<()>),
    Reply(u64, Reply),
    Gone(u64),
}

/// The session and the one worker process it may have.
pub struct Shell {
    app: AppHandle,
    tx: Sender<Msg>,
    session: Session,
    worker: Option<Worker>,
}

impl Shell {
    /// A shell with no worker. Inputs: the app, and the channel the worker's reader reports on.
    pub fn new(app: AppHandle, tx: Sender<Msg>) -> Self {
        Shell { app, tx, session: Session::new(Instant::now()), worker: None }
    }

    /// Feeds one session event and everything it causes (a failed start or a full queue come back as events).
    fn handle(&mut self, event: Event) {
        let mut events = VecDeque::from([event]);
        while let Some(event) = events.pop_front() {
            for effect in self.session.step(event, Instant::now()) {
                self.run_effect(effect, &mut events);
            }
        }
    }

    /// Runs one effect; anything that goes wrong is queued as an event for the session.
    fn run_effect(&mut self, effect: Effect, events: &mut VecDeque<Event>) {
        match effect {
            Effect::Spawn { serial, pick } => {
                if let Err(message) = self.start(serial, pick) {
                    events.push_back(Event::SpawnFailed { serial, message });
                }
            }
            Effect::Send { serial, request, samples } => {
                let sent = self.worker.as_ref().filter(|w| w.serial == serial).map(|w| w.send(request, samples));
                if sent.is_some_and(|r| r.is_err()) {
                    events.push_back(Event::Overload { serial });
                }
            }
            Effect::Kill { serial } => {
                if let Some(worker) = self.worker.take_if(|w| w.serial == serial) {
                    worker.kill();
                }
            }
            Effect::Deliver(done, result) => deliver(done, result),
            Effect::Report(message) => crate::status::report(&self.app, &message),
            Effect::Partial { id, index, text } => super::emit_partial(id, index, &text),
            Effect::Trim => crate::mem::trim_soon(),
            Effect::Log(line) => crate::log::note(&line),
        }
    }

    /// Starts the worker process and queues its model load. Inputs: its serial and the model. Returns why it could not start.
    fn start(&mut self, serial: u64, pick: Pick) -> Result<(), String> {
        let model = catalog::find(pick.id).ok_or(NO_MODEL)?;
        let path = |p: Option<std::path::PathBuf>| p.map(|p| p.to_string_lossy().replace('\\', "/")).ok_or(NO_MODEL);
        let (dir, vad) = (path(catalog::dir(&self.app, model))?, path(catalog::vad_path(&self.app))?);
        let tx = self.tx.clone();
        let worker = process::spawn(serial, move |reply| {
            let _ = tx.send(reply.map_or(Msg::Gone(serial), |r| Msg::Reply(serial, r)));
        })?;
        let load = Request::Load { dir, vad, spec: pick.spec.clone(), threads: process::threads(), version: VERSION };
        let _ = worker.send(load, Vec::new());
        self.worker = Some(worker);
        Ok(())
    }

    /// Reads the idle-unload setting for the session. Done when a dictation starts or stops, not on every audio chunk or reply.
    fn read_unload(&mut self) {
        self.session.set_unload(Unload::from_setting(crate::settings::get_i64("unload_after_secs", 120)));
    }

    /// One turn of the loop: a message, or None for the clock passing a deadline.
    fn turn(&mut self, msg: Option<Msg>) {
        match msg {
            Some(msg) => self.message(msg),
            None => self.handle(Event::Tick),
        }
    }

    /// After a panic: stops the session like an unload, so the worker is killed and every parked dictation gets its callback (what was
    /// recognised so far, or the stopped message), then makes sure no worker process is left. The next dictation starts clean.
    fn recover(&mut self) {
        let _ = guard::contain(|| self.handle(Event::Unload));
        if let Some(worker) = self.worker.take() {
            worker.kill();
        }
    }

    /// Turns a message into a session event and handles it; an Unload is acknowledged once the worker is gone.
    fn message(&mut self, msg: Msg) {
        match msg {
            Msg::Begin(id) => {
                self.read_unload();
                let model = catalog::pick(&self.app).map(|m| Pick { id: m.id, spec: &m.engine }).ok_or_else(|| NO_MODEL.to_string());
                let cap = Duration::from_secs(crate::settings::get_u64("max_record_secs", 1200).clamp(30, 3600));
                self.handle(Event::Begin { id, model, cap });
            }
            Msg::Audio(id, samples) => self.handle(Event::Audio { id, samples }),
            Msg::End(id, done) => {
                self.read_unload();
                self.handle(Event::End { id, done });
            }
            Msg::Cancel(id) => self.handle(Event::Cancel { id }),
            Msg::Unload(ack) => {
                self.handle(Event::Unload);
                let _ = ack.send(());
            }
            Msg::Reply(serial, reply) => self.handle(Event::Reply { serial, reply }),
            Msg::Gone(serial) => self.handle(Event::Gone { serial }),
        }
    }
}

/// Runs a dictation's callback; a panic in it is contained so the callbacks after it still run. Inputs: the callback and what it is given.
fn deliver(done: Done, result: Result<String, String>) {
    let _ = guard::contain(|| done(result));
}

/// Runs `step` on `state` with `input`; when it panics, runs `recover` on the same state (also contained) so the loop carries on.
/// Inputs: the state, the step's input, the step and the cleanup.
fn guarded<S, I>(state: &mut S, input: I, step: fn(&mut S, I), recover: fn(&mut S)) {
    if guard::contain(|| step(state, input)).is_none() {
        let _ = guard::contain(|| recover(state));
    }
}

/// The thread loop. Blocks until a message or the session's deadline; when nothing is loaded or in flight it asks `retire` for a last message
/// that raced in and otherwise returns, so an idle Cue holds no speech thread. Inputs: the shell, its channel and the retire handshake.
pub fn run(mut shell: Shell, rx: Receiver<Msg>, retire: impl Fn(&Receiver<Msg>) -> Option<Msg>) {
    loop {
        let msg = if shell.session.quiescent() {
            match retire(&rx) {
                Some(msg) => msg,
                None => return,
            }
        } else {
            match shell.session.deadline() {
                Some(at) => match rx.recv_timeout(at.saturating_duration_since(Instant::now())) {
                    Ok(msg) => msg,
                    Err(RecvTimeoutError::Timeout) => {
                        guarded(&mut shell, None, Shell::turn, Shell::recover);
                        continue;
                    }
                    Err(RecvTimeoutError::Disconnected) => return,
                },
                None => match rx.recv() {
                    Ok(msg) => msg,
                    Err(_) => return,
                },
            }
        };
        guarded(&mut shell, Some(msg), Shell::turn, Shell::recover);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;

    /// A panicking callback does not stop the callbacks delivered after it.
    #[test]
    fn a_panicking_done_does_not_lose_the_others() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let record = |name: &'static str| {
            let seen = seen.clone();
            Box::new(move |_: Result<String, String>| seen.lock().unwrap().push(name)) as Done
        };
        deliver(record("first"), Ok(String::new()));
        deliver(Box::new(|_| panic!("caller bug")), Ok(String::new()));
        deliver(record("last"), Err(String::new()));
        assert_eq!(*seen.lock().unwrap(), ["first", "last"]);
    }

    /// A panic in a step runs the cleanup on the same state (where the worker is killed); a step that does not panic runs no cleanup.
    #[test]
    fn a_panicking_step_runs_the_cleanup() {
        let mut killed = false;
        guarded(&mut killed, true, |_, boom| assert!(!boom, "step bug"), |k| *k = true);
        assert!(killed);
        let mut killed = false;
        guarded(&mut killed, false, |_, boom| assert!(!boom), |k| *k = true);
        assert!(!killed);
    }
}
