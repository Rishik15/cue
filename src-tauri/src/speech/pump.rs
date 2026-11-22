//! File: speech/pump.rs
//! Purpose: The speech thread: the thin shell around `session`. It turns what other threads send into session events, runs the effects the session
//! answers with (start or kill the worker process, write to its pipe, call the dictation's callback, tell the user), and blocks on the channel
//! with the session's deadline. No polling: nothing runs while Cue is idle, and the thread leaves when there is nothing left to do.
//! Contents: Msg — what other threads send; Host — what the shell needs from the app (the seam tests replace); Shell — the session plus the worker process; refusal — the event a refused send causes; guarded — one step whose panic is contained and cleaned up after; run — the thread loop.

use std::collections::VecDeque;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use tauri::AppHandle;

use super::catalog;
use super::events::{Effect, Event, Pick, Unload, DELETING, NO_MODEL};
use super::process::{self, Refused, Worker};
use super::protocol::{Reply, Request, VERSION};
use super::session::Session;
use super::take::{Done, Failure};
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
    /// Kill the worker if it runs this model and answer once it is gone (the caller is about to delete the model's files).
    Evict(&'static str, Sender<()>),
    Reply(u64, Reply),
    Gone(u64),
}

/// What the shell needs from the app; the real one is the `AppHandle`, a test brings its own.
pub trait Host: Send + 'static {
    /// Tells the user `message`.
    fn report(&self, message: &str);
    /// The installed model a dictation should load, if any.
    fn pick(&self) -> Option<Pick>;
    /// Starts the worker process for `pick`. Inputs: its serial, the model and the callback for its replies. Returns it or why it could not start.
    fn spawn(&self, serial: u64, pick: &Pick, on_reply: Box<dyn Fn(Option<Reply>) + Send + Sync>) -> Result<Worker, String>;
}

impl Host for AppHandle {
    /// Shows `message` through the app's status line.
    fn report(&self, message: &str) {
        crate::status::report(self, message);
    }

    /// The user's chosen installed model as a `Pick`, or None when none is installed.
    fn pick(&self) -> Option<Pick> {
        catalog::pick(self).map(|m| Pick { id: m.id, spec: &m.engine })
    }

    /// Spawns the real worker process for `pick` with its serial and reply callback, and queues its model load. Returns the worker, or why it could not start.
    fn spawn(&self, serial: u64, pick: &Pick, on_reply: Box<dyn Fn(Option<Reply>) + Send + Sync>) -> Result<Worker, String> {
        let model = catalog::find(pick.id).ok_or(NO_MODEL)?;
        let path = |p: Option<std::path::PathBuf>| p.map(|p| p.to_string_lossy().replace('\\', "/")).ok_or(NO_MODEL);
        let (dir, vad) = (path(catalog::dir(self, model))?, path(catalog::vad_path(self))?);
        let worker = process::spawn(serial, on_reply).map_err(|e| e.to_string())?;
        let load = Request::Load { dir, vad, spec: pick.spec.clone(), threads: process::threads(), version: VERSION };
        if worker.send(load, Vec::new()).is_err() {
            worker.kill(); // its pipe is already shut: no model will ever load
            return Err("The speech engine closed its input before it could load.".into());
        }
        Ok(worker)
    }
}

/// The session and the one worker process it may have.
pub struct Shell<H: Host = AppHandle> {
    app: H,
    tx: Sender<Msg>,
    session: Session,
    worker: Option<Worker>,
}

impl<H: Host> Shell<H> {
    /// A shell with no worker. Inputs: the host, and the channel the worker's reader reports on.
    pub fn new(app: H, tx: Sender<Msg>) -> Self {
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
                events.extend(refusal(serial, sent));
            }
            Effect::Kill { serial } => {
                if let Some(worker) = self.worker.take_if(|w| w.serial == serial) {
                    worker.kill();
                }
            }
            Effect::Deliver(done, result) => deliver(done, result),
            Effect::Report(message) => self.app.report(&message),
            Effect::Partial { id, index, text } => super::emit_partial(id, index, &text),
            Effect::Failed { id } => super::emit_failed(id),
            Effect::Trim => crate::mem::trim_soon(),
            Effect::Log(line) => crate::log::note(&line),
        }
    }

    /// Starts the worker process (which queues its model load). Inputs: its serial and the model. Returns why it could not start.
    fn start(&mut self, serial: u64, pick: Pick) -> Result<(), String> {
        let tx = self.tx.clone();
        let on_reply = move |reply: Option<Reply>| {
            let _ = tx.send(reply.map_or(Msg::Gone(serial), |r| Msg::Reply(serial, r)));
        };
        self.worker = Some(self.app.spawn(serial, &pick, Box::new(on_reply))?);
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
                let model = self.app.pick().ok_or_else(|| if super::is_deleting() { DELETING } else { NO_MODEL }.to_string());
                let cap = Duration::from_secs(crate::settings::get_u64("max_record_secs", 1200).clamp(30, 3600));
                self.handle(Event::Begin { id, model, cap });
            }
            Msg::Audio(id, samples) => self.handle(Event::Audio { id, samples }),
            Msg::End(id, done) => {
                self.read_unload();
                self.handle(Event::End { id, done });
            }
            Msg::Cancel(id) => self.handle(Event::Cancel { id }),
            Msg::Evict(id, ack) => {
                self.handle(Event::Evict { id });
                let _ = ack.send(());
            }
            Msg::Reply(serial, reply) => self.handle(Event::Reply { serial, reply }),
            Msg::Gone(serial) => self.handle(Event::Gone { serial }),
        }
    }
}

/// The event a refused send causes. Inputs: the worker's serial and what its queue answered (None: that worker is no longer the shell's).
/// Returns Overload for a full queue, Gone for a closed writer, and nothing otherwise.
fn refusal(serial: u64, sent: Option<Result<(), Refused>>) -> Option<Event> {
    match sent {
        Some(Err(Refused::Full)) => Some(Event::Overload { serial }),
        Some(Err(Refused::Gone)) => Some(Event::Gone { serial }),
        _ => None,
    }
}

/// Runs a dictation's callback; a panic in it is contained so the callbacks after it still run. Inputs: the callback and what it is given.
fn deliver(done: Done, result: Result<String, Failure>) {
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
pub fn run<H: Host>(mut shell: Shell<H>, rx: Receiver<Msg>, retire: impl Fn(&Receiver<Msg>) -> Option<Msg>) {
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
    use std::cell::Cell;
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::slot::Slot;
    use crate::speech::catalog::all;

    /// A host that starts no process: `spawn` fails with `fail` and every report is kept.
    struct FakeHost {
        fail: &'static str,
        reports: Arc<Mutex<Vec<String>>>,
    }

    impl Host for FakeHost {
        /// Keeps `message` for the test to read.
        fn report(&self, message: &str) {
            self.reports.lock().unwrap().push(message.into());
        }

        /// Offers no model, so a Begin fails.
        fn pick(&self) -> Option<Pick> {
            None
        }

        /// Always fails with the host's `fail` text; no process starts.
        fn spawn(&self, _: u64, _: &Pick, _: Box<dyn Fn(Option<Reply>) + Send + Sync>) -> Result<Worker, String> {
            Err(self.fail.into())
        }
    }

    /// A shell on the fake host, and the reports it will collect.
    fn shell() -> (Shell<FakeHost>, Arc<Mutex<Vec<String>>>) {
        let reports = Arc::new(Mutex::new(Vec::new()));
        let host = FakeHost { fail: "cannot start", reports: reports.clone() };
        (Shell::new(host, std::sync::mpsc::channel().0), reports)
    }

    /// Runs one effect on the shell and returns the events it queued.
    fn run_one(shell: &mut Shell<FakeHost>, effect: Effect) -> Vec<Event> {
        let mut events = VecDeque::new();
        shell.run_effect(effect, &mut events);
        events.into()
    }

    /// A Kill ends only the worker with its serial: a late Kill for an older worker leaves the current one running.
    #[test]
    fn a_kill_for_another_serial_leaves_the_worker() {
        let (mut shell, _) = shell();
        shell.worker = Some(process::idle_worker(2));
        assert!(run_one(&mut shell, Effect::Kill { serial: 1 }).is_empty());
        assert_eq!(shell.worker.as_ref().map(|w| w.serial), Some(2));
        assert!(run_one(&mut shell, Effect::Kill { serial: 2 }).is_empty());
        assert!(shell.worker.is_none());
    }

    /// A Send to a worker the shell no longer has causes no event; a spawn that fails comes back as SpawnFailed for its serial, and a Report reaches the host.
    #[test]
    fn effects_report_their_failures_as_events() {
        let (mut shell, reports) = shell();
        let send = Effect::Send { serial: 9, request: Request::End { id: 1 }, samples: Vec::new() };
        assert!(run_one(&mut shell, send).is_empty());
        let pick = Pick { id: all().next().unwrap().id, spec: &all().next().unwrap().engine };
        let events = run_one(&mut shell, Effect::Spawn { serial: 4, pick });
        assert!(matches!(&events[..], [Event::SpawnFailed { serial: 4, message }] if message == "cannot start"));
        run_one(&mut shell, Effect::Report("hello".into()));
        assert_eq!(*reports.lock().unwrap(), ["hello"]);
    }

    /// An idle shell asks `retire` for a last message: one that raced in is handled and the loop asks again, and the loop returns only when
    /// `retire` has nothing.
    #[test]
    fn an_idle_loop_handles_the_message_that_raced_in_then_retires() {
        let (shell, _) = shell();
        let (ack, acked) = std::sync::mpsc::channel();
        let asked = Cell::new(0);
        let last = Cell::new(Some(Msg::Evict(all().next().unwrap().id, ack)));
        run(shell, std::sync::mpsc::channel().1, |_| {
            asked.set(asked.get() + 1);
            last.take()
        });
        assert_eq!(asked.get(), 2, "asked once for the raced message, once more after it");
        assert!(acked.try_recv().is_ok(), "the raced message was handled");
    }

    /// The real thread handoff: a dictation through a `Slot` runs, the idle thread retires, and the next dictation starts a new thread on the
    /// same slot whose callback still fires.
    #[test]
    fn a_retired_thread_is_restarted_by_the_next_dictation() {
        let slot = Arc::new(Slot::<Msg>::new());
        let starts = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let (out, results) = std::sync::mpsc::channel();
        let (retired, retirements) = std::sync::mpsc::channel();
        for id in 1..=2u64 {
            let (spawn_slot, count, retired) = (slot.clone(), starts.clone(), retired.clone());
            let spawn = move |tx: Sender<Msg>, rx: Receiver<Msg>| {
                count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let host = FakeHost { fail: "cannot start", reports: Arc::default() };
                let shell = Shell::new(host, tx);
                std::thread::spawn(move || {
                    run(shell, rx, |rx| spawn_slot.retire(rx));
                    retired.send(()).unwrap();
                });
                Ok(())
            };
            assert!(slot.send(Msg::Begin(id), spawn).is_ok());
            let out = out.clone();
            let done: Done = Box::new(move |r| out.send((id, r.is_err())).unwrap());
            assert!(slot.try_send(Msg::End(id, done)).is_ok(), "the thread stays while the dictation is in flight");
            assert_eq!(results.recv_timeout(Duration::from_secs(5)).unwrap(), (id, true));
            retirements.recv_timeout(Duration::from_secs(5)).expect("the idle thread retired");
            assert!(slot.try_send(Msg::Cancel(0)).is_err(), "the idle thread retired and emptied the slot");
        }
        assert_eq!(starts.load(std::sync::atomic::Ordering::SeqCst), 2, "one thread per dictation after a retirement");
    }

    /// A full queue is an Overload and a closed writer is Gone, both for the worker that was written to; a send that went through, or to a
    /// worker the shell no longer has, causes nothing.
    #[test]
    fn a_refused_send_becomes_the_event_for_its_worker() {
        assert!(matches!(refusal(3, Some(Err(Refused::Full))), Some(Event::Overload { serial: 3 })));
        assert!(matches!(refusal(4, Some(Err(Refused::Gone))), Some(Event::Gone { serial: 4 })));
        assert!(refusal(5, Some(Ok(()))).is_none());
        assert!(refusal(6, None).is_none());
    }

    /// A panicking callback does not stop the callbacks delivered after it.
    #[test]
    fn a_panicking_done_does_not_lose_the_others() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let record = |name: &'static str| {
            let seen = seen.clone();
            Box::new(move |_: Result<String, Failure>| seen.lock().unwrap().push(name)) as Done
        };
        deliver(record("first"), Ok(String::new()));
        deliver(Box::new(|_| panic!("caller bug")), Ok(String::new()));
        deliver(record("last"), Err(String::new().into()));
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
