//! File: text/engine.rs
//! Purpose: The life of the text model's server as one state machine: start on first use, reuse while held, stop when idle, restart on demand, stop for a model delete, and abandon a cold start the caller no longer wants.
//! Contents: Engine — acquire / stop_for_delete / keeps_warm over one `Mutex<State>`; Lease — a hold on a running server; Aborter — a handle that can end that server without holding it; DeleteGuard — new calls are refused while a model is deleted; Host and Child — the process seam; Clock — the time seam; Fail / Probe / Conn — results.
//! Invariants: Stopped, Starting, Running and Stopping are the only states and every change happens under the one lock, each carrying a new generation. A lease, an idle watcher and a stop all name the generation they act on, so one that is late finds a different generation and touches nothing.
//! The idle watcher is one thread that waits on a timed condition and is part of the state (`watcher` holds the generation it serves). Nothing runs while the server is stopped.

use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::Duration;

use super::cancel::Cancel;

/// How often a waiter looks again whether it is still wanted, and how often a cold start asks the server whether it has loaded.
pub const POLL: Duration = Duration::from_millis(100);

/// Longest wait for the model to load. Seconds on an SSD; a hard disk with a 2-thread CPU can need minutes, so the limit is generous.
pub const READY_LIMIT: Duration = Duration::from_secs(240);

const REMOVING: &str = "The text model is being removed.";

/// How to reach the running server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Conn {
    pub port: u16,
    pub key: String,
}

/// Why a call to the server did not produce a lease.
#[derive(Debug, PartialEq, Eq)]
pub enum Fail {
    Cancelled,
    Failed(String),
}

/// What a started server says when asked whether it is ready.
pub enum Probe {
    Loading,
    Ready(Conn),
    Failed(String),
}

/// One server process. `kill` ends it and waits, so its model file is released when it returns.
pub trait Child: Send + 'static {
    /// Whether this is still the live, trusted server (the process runs and owns its port).
    fn alive(&mut self) -> bool;
    /// Asks whether the server has loaded the model yet.
    fn probe(&mut self) -> Probe;
    /// Ends the process and waits for it.
    fn kill(&mut self);
}

/// Where processes, settings and threads come from: the real one starts llama.cpp, a test one is scripted.
pub trait Host: Send + Sync + 'static {
    type Child: Child;
    /// Starts a server for `model` without waiting for it to load; `fallback` asks for the safer way (CPU only). Returns the process or the reason it did not start.
    fn launch(&self, model: &'static str, fallback: bool) -> Result<Self::Child, String>;
    /// Seconds a server stays loaded after its last request: 0 stops it at once, below 0 never.
    fn idle_secs(&self) -> i64;
    /// Whether a failed start is worth one more try with `fallback`.
    fn retry_after_failure(&self) -> bool;
    /// Called when the fallback start worked, so later starts can skip the failing way.
    fn fallback_worked(&self);
    /// Runs `body` on a new thread. Returns false when the thread could not start.
    fn spawn(&self, body: Box<dyn FnOnce() + Send>) -> bool;
    /// Records a line for the support log.
    fn note(&self, line: &str);
}

/// Time as the engine sees it, so tests can move it by hand.
pub trait Clock: Send + Sync + 'static {
    /// Time since an arbitrary start.
    fn now(&self) -> Duration;
    /// Sleeps for `d` (a cold start waits between two questions to the server).
    fn pause(&self, d: Duration);
    /// Waits on `cv` for at most `d`, returning the lock again.
    fn wait<'a, T>(&self, cv: &Condvar, guard: MutexGuard<'a, T>, d: Duration) -> MutexGuard<'a, T>;
}

/// The real clock: `Instant` and the OS sleep.
pub struct SystemClock(std::time::Instant);

impl SystemClock {
    /// A clock whose zero is now.
    pub fn new() -> Self {
        Self(std::time::Instant::now())
    }
}

impl Clock for SystemClock {
    /// Time since the clock was made.
    fn now(&self) -> Duration {
        self.0.elapsed()
    }

    /// Sleeps the thread.
    fn pause(&self, d: Duration) {
        std::thread::sleep(d);
    }

    /// A timed condition wait; a poisoned lock is used as it is.
    fn wait<'a, T>(&self, cv: &Condvar, guard: MutexGuard<'a, T>, d: Duration) -> MutexGuard<'a, T> {
        cv.wait_timeout(guard, d).unwrap_or_else(|e| e.into_inner()).0
    }
}

struct Live<C> {
    child: C,
    conn: Conn,
    model: &'static str,
    leases: usize,
    last_used: Duration,
}

enum Phase<C> {
    Stopped,
    Starting,
    Running(Live<C>),
    Stopping,
}

struct State<C> {
    phase: Phase<C>,
    gen: u64,
    watcher: Option<u64>,
    removing: bool,
}

enum Step {
    Lease(Conn),
    Replace,
    Start,
    Wait,
}

enum Idle {
    Retire,
    Expired,
    Wait(Duration),
}

/// The server's life, shared by every caller. Create it once and keep it in an `Arc`.
pub struct Engine<H: Host, C: Clock> {
    host: H,
    clock: C,
    state: Mutex<State<H::Child>>,
    changed: Condvar,
}

/// A request's hold on the running server. While it lives the idle watcher and "Immediately" leave the server alone; dropping it restarts the idle clock.
pub struct Lease<H: Host, C: Clock> {
    engine: Arc<Engine<H, C>>,
    gen: u64,
    pub conn: Conn,
}

impl<H: Host, C: Clock> Drop for Lease<H, C> {
    /// Gives the hold back.
    fn drop(&mut self) {
        self.engine.release(self.gen);
    }
}

impl<H: Host, C: Clock> Lease<H, C> {
    /// A handle that can end the server this lease holds, even though it is held, from another thread without keeping the server alive: the request using it is abandoned and fails at once. Returns the handle.
    pub fn aborter(&self) -> Aborter<H, C> {
        Aborter { engine: Arc::clone(&self.engine), gen: self.gen }
    }
}

/// The right to end one server generation, held by a watchdog thread. It is not a lease: it never keeps the server alive.
pub struct Aborter<H: Host, C: Clock> {
    engine: Arc<Engine<H, C>>,
    gen: u64,
}

impl<H: Host, C: Clock> Aborter<H, C> {
    /// Ends the server of the generation this handle was made for; does nothing when it was already replaced.
    pub fn abort(&self) {
        self.engine.abort(self.gen);
    }
}

/// While it lives no new call can reach the server, so a model file can be deleted. Dropping it lets calls through again.
pub struct DeleteGuard<H: Host, C: Clock>(Arc<Engine<H, C>>);

impl<H: Host, C: Clock> Drop for DeleteGuard<H, C> {
    /// Lets new calls through.
    fn drop(&mut self) {
        self.0.lock().removing = false;
        self.0.changed.notify_all();
    }
}

impl<H: Host, C: Clock> Engine<H, C> {
    /// An engine with nothing running. Inputs: where processes come from and the clock. Returns it.
    pub fn new(host: H, clock: C) -> Arc<Self> {
        let state = State { phase: Phase::Stopped, gen: 0, watcher: None, removing: false };
        Arc::new(Self { host, clock, state: Mutex::new(state), changed: Condvar::new() })
    }

    fn lock(&self) -> MutexGuard<'_, State<H::Child>> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Whether the server stays loaded between requests, so warming it ahead of one is worth it.
    pub fn keeps_warm(&self) -> bool {
        self.host.idle_secs() != 0
    }

    /// A lease on a server running `model`. Starts one when none runs or it died, replaces one that holds another model once that is free, waits for a stop in progress,
    /// and gives up with `Cancelled` the moment `cancel` is set, killing a server that is still loading.
    pub fn acquire(self: &Arc<Self>, model: &'static str, cancel: &Cancel) -> Result<Lease<H, C>, Fail> {
        let mut st = self.lock();
        loop {
            if st.removing {
                return Err(Fail::Failed(REMOVING.into()));
            }
            if cancel.is_set() {
                return Err(Fail::Cancelled);
            }
            match self.next_step(&mut st, model) {
                Step::Lease(conn) => {
                    self.ensure_watcher(&mut st);
                    return Ok(Lease { engine: Arc::clone(self), gen: st.gen, conn });
                }
                Step::Replace => st = self.retire(st),
                Step::Start => return self.cold_start(st, model, cancel),
                Step::Wait => st = self.clock.wait(&self.changed, st, POLL),
            }
        }
    }

    /// Decides what a caller wanting `model` does next, and counts the lease when it can have one.
    fn next_step(&self, st: &mut State<H::Child>, model: &'static str) -> Step {
        match &mut st.phase {
            Phase::Stopped => Step::Start,
            Phase::Starting | Phase::Stopping => Step::Wait,
            Phase::Running(live) => {
                let alive = live.child.alive();
                if alive && live.model == model {
                    live.leases += 1;
                    Step::Lease(live.conn.clone())
                } else if live.leases == 0 || !alive {
                    Step::Replace
                } else {
                    Step::Wait
                }
            }
        }
    }

    /// Starts the server with the lock released but `Starting` published, so a stop or another caller has a state to wait on. Returns the lease for the caller that started it.
    fn cold_start(
        self: &Arc<Self>,
        mut st: MutexGuard<'_, State<H::Child>>,
        model: &'static str,
        cancel: &Cancel,
    ) -> Result<Lease<H, C>, Fail> {
        st.gen += 1;
        st.phase = Phase::Starting;
        let gen = st.gen;
        drop(st);
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.start(model, cancel)))
            .unwrap_or_else(|_| Err(Fail::Failed("The text engine failed to start.".into())));
        let mut st = self.lock();
        let result = match outcome {
            Ok((child, conn)) => {
                let live = Live { child, conn: conn.clone(), model, leases: 1, last_used: self.clock.now() };
                st.phase = Phase::Running(live);
                self.ensure_watcher(&mut st);
                Ok(Lease { engine: Arc::clone(self), gen, conn })
            }
            Err(fail) => {
                st.phase = Phase::Stopped;
                Err(fail)
            }
        };
        self.changed.notify_all();
        result
    }

    /// Starts the server the usual way and, when that fails and the host allows, once more the safe way.
    fn start(&self, model: &'static str, cancel: &Cancel) -> Result<(H::Child, Conn), Fail> {
        match self.attempt(model, false, cancel) {
            Err(Fail::Failed(why)) if self.host.retry_after_failure() => {
                self.host.note(&format!("text engine failed ({why}); trying the safe way"));
                let second = self.attempt(model, true, cancel);
                if second.is_ok() {
                    self.host.fallback_worked();
                }
                second
            }
            other => other,
        }
    }

    /// One launch and the wait for the model to load. A cancel or a failure kills the process before returning.
    fn attempt(&self, model: &'static str, fallback: bool, cancel: &Cancel) -> Result<(H::Child, Conn), Fail> {
        let mut child = self.host.launch(model, fallback).map_err(Fail::Failed)?;
        let began = self.clock.now();
        loop {
            if cancel.is_set() {
                child.kill();
                return Err(Fail::Cancelled);
            }
            match child.probe() {
                Probe::Ready(conn) => return Ok((child, conn)),
                Probe::Failed(why) => {
                    child.kill();
                    return Err(Fail::Failed(why));
                }
                Probe::Loading => {}
            }
            if self.clock.now().saturating_sub(began) >= READY_LIMIT {
                child.kill();
                return Err(Fail::Failed("The text engine took too long to start.".into()));
            }
            self.clock.pause(POLL);
        }
    }

    /// Takes the running server out, publishes `Stopping`, kills it with the lock released and publishes `Stopped`. A caller wanting a server waits for that, so a replacement is never confused with the old process. Returns the lock.
    fn retire<'a>(&'a self, mut st: MutexGuard<'a, State<H::Child>>) -> MutexGuard<'a, State<H::Child>> {
        let Phase::Running(mut live) = std::mem::replace(&mut st.phase, Phase::Stopping) else {
            return st;
        };
        st.gen += 1;
        st.watcher = None;
        let gen = st.gen;
        self.changed.notify_all();
        drop(st);
        live.child.kill();
        let mut st = self.lock();
        if st.gen == gen {
            st.phase = Phase::Stopped;
        }
        self.changed.notify_all();
        st
    }

    /// Gives a lease back and restarts the idle clock; stops the server at once when the setting is "Immediately" and nothing else holds it.
    fn release(self: &Arc<Self>, gen: u64) {
        let mut st = self.lock();
        let (now, immediate) = (self.clock.now(), self.host.idle_secs() == 0);
        let state = &mut *st;
        let free = match &mut state.phase {
            Phase::Running(live) if state.gen == gen => {
                live.leases = live.leases.saturating_sub(1);
                live.last_used = now;
                live.leases == 0
            }
            _ => return,
        };
        if free && immediate {
            drop(self.retire(st));
            return;
        }
        self.ensure_watcher(&mut st);
        self.changed.notify_all();
    }

    /// Ends the server of generation `gen` whether or not it is held.
    fn abort(&self, gen: u64) {
        let st = self.lock();
        if st.gen == gen {
            drop(self.retire(st));
        }
    }

    /// Refuses new calls, waits for the running ones to finish, and stops the server, so the model files can be deleted. Returns the guard that lets calls through again.
    pub fn stop_for_delete(self: &Arc<Self>) -> DeleteGuard<H, C> {
        let mut st = self.lock();
        st.removing = true;
        loop {
            match &st.phase {
                Phase::Stopped => break,
                Phase::Running(live) if live.leases == 0 => st = self.retire(st),
                _ => st = self.clock.wait(&self.changed, st, POLL),
            }
        }
        drop(st);
        DeleteGuard(Arc::clone(self))
    }

    /// Starts the one idle watcher for the running server when the setting wants one and none serves this generation.
    fn ensure_watcher(self: &Arc<Self>, st: &mut State<H::Child>) {
        if self.host.idle_secs() <= 0 || st.watcher == Some(st.gen) || !matches!(st.phase, Phase::Running(_)) {
            return;
        }
        let (engine, gen) = (Arc::clone(self), st.gen);
        if self.host.spawn(Box::new(move || engine.watch(gen))) {
            st.watcher = Some(gen);
        }
    }

    /// The idle watcher for generation `gen`: waits on a deadline, stops the server when it has been idle long enough, and retires without touching anything when the generation is no longer the running one.
    fn watch(self: &Arc<Self>, gen: u64) {
        let mut st = self.lock();
        loop {
            match self.idle_step(&st, gen) {
                Idle::Wait(d) => st = self.clock.wait(&self.changed, st, d),
                Idle::Expired => {
                    drop(self.retire(st));
                    return;
                }
                Idle::Retire => {
                    if st.watcher == Some(gen) {
                        st.watcher = None;
                    }
                    return;
                }
            }
        }
    }

    /// What the watcher of `gen` does now: retire (not the running generation, or the setting no longer wants a timer), stop (idle long enough and free) or wait.
    fn idle_step(&self, st: &State<H::Child>, gen: u64) -> Idle {
        let Phase::Running(live) = &st.phase else { return Idle::Retire };
        let secs = self.host.idle_secs();
        if st.gen != gen || secs <= 0 {
            return Idle::Retire;
        }
        let idle = Duration::from_secs(secs.unsigned_abs());
        if live.leases > 0 {
            return Idle::Wait(idle);
        }
        let since = self.clock.now().saturating_sub(live.last_used);
        if since >= idle {
            Idle::Expired
        } else {
            Idle::Wait(idle - since)
        }
    }

    /// The state in words and the generation the watcher serves, for tests.
    #[cfg(test)]
    pub fn snapshot(&self) -> (&'static str, Option<u64>, u64) {
        let st = self.lock();
        let name = match st.phase {
            Phase::Stopped => "stopped",
            Phase::Starting => "starting",
            Phase::Running(_) => "running",
            Phase::Stopping => "stopping",
        };
        (name, st.watcher, st.gen)
    }
}
