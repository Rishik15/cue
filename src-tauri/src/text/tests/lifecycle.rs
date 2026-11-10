//! File: text/tests/lifecycle.rs
//! Purpose: Protect the life of the model server with a scripted process and a clock moved by hand: reuse, idle stop, a restart that races an idle stop, delete exclusion, failed starts and a cancelled cold start.
//! Contents: Fake host, child and clock; tests of `Engine` (idle 0, never, lease held across idle, restart race, delete, start failure, GPU fallback, cancel while starting, a panic during a start, a watcher that cannot start, replacing another model or a dead process, retire in other phases).

use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Barrier, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use crate::text::cancel::Cancel;
use crate::text::engine::{Child, Clock, Conn, Engine, Fail, Host, Probe};

const MODEL: &str = "model-a";

/// What the fake world records and what a test scripts.
#[derive(Default)]
struct World {
    idle: AtomicI64,
    launches: AtomicUsize,
    alive: AtomicUsize,
    kills: AtomicUsize,
    loading_polls: AtomicUsize,
    fail_launch: AtomicBool,
    fail_first_probe: AtomicBool,
    dead: AtomicBool,
    spawn_fails: AtomicBool,
    panic_on_probe: AtomicBool,
    retry_allowed: AtomicBool,
    fallback_recorded: AtomicBool,
    kill_gate: Mutex<Option<Arc<Barrier>>>,
    now_ms: AtomicU64,
}

/// A host whose processes are counters in `World`.
struct FakeHost(Arc<World>);

/// One fake process; `loads_after` is how many probes say "loading" first.
struct FakeChild {
    world: Arc<World>,
    polls_left: usize,
    broken: bool,
}

impl Child for FakeChild {
    /// Alive until killed, or until the test marks processes dead.
    fn alive(&mut self) -> bool {
        !self.world.dead.load(Ordering::SeqCst)
    }

    /// Loading for the scripted number of polls, then ready (or failed once when scripted).
    fn probe(&mut self) -> Probe {
        if self.world.panic_on_probe.load(Ordering::SeqCst) {
            panic!("scripted panic");
        }
        if self.broken {
            return Probe::Failed("scripted failure".into());
        }
        if self.polls_left > 0 {
            self.polls_left -= 1;
            return Probe::Loading;
        }
        Probe::Ready(Conn { port: 1, key: "k".into() })
    }

    /// Counts the kill, after waiting at the test's barrier when it set one.
    fn kill(&mut self) {
        let gate = self.world.kill_gate.lock().unwrap().clone();
        if let Some(gate) = gate {
            gate.wait();
            gate.wait();
        }
        self.world.alive.fetch_sub(1, Ordering::SeqCst);
        self.world.kills.fetch_add(1, Ordering::SeqCst);
    }
}

impl Host for FakeHost {
    type Child = FakeChild;

    /// Starts a counted fake process, or fails when scripted. The first launch is the broken one when `fail_first_probe` is set and this is not the fallback.
    fn launch(&self, _model: &'static str, fallback: bool) -> Result<FakeChild, String> {
        let w = &self.0;
        w.launches.fetch_add(1, Ordering::SeqCst);
        if w.fail_launch.load(Ordering::SeqCst) {
            return Err("no process".into());
        }
        w.alive.fetch_add(1, Ordering::SeqCst);
        let broken = w.fail_first_probe.load(Ordering::SeqCst) && !fallback;
        Ok(FakeChild { world: Arc::clone(w), polls_left: w.loading_polls.load(Ordering::SeqCst), broken })
    }

    /// The scripted idle setting.
    fn idle_secs(&self) -> i64 {
        self.0.idle.load(Ordering::SeqCst)
    }

    /// The scripted retry permission.
    fn retry_after_failure(&self) -> bool {
        self.0.retry_allowed.load(Ordering::SeqCst)
    }

    /// Records that the fallback worked.
    fn fallback_worked(&self) {
        self.0.fallback_recorded.store(true, Ordering::SeqCst);
    }

    /// A plain thread, or a refusal when scripted.
    fn spawn(&self, body: Box<dyn FnOnce() + Send>) -> bool {
        if self.0.spawn_fails.load(Ordering::SeqCst) {
            return false;
        }
        std::thread::spawn(body);
        true
    }

    /// Ignores the line.
    fn note(&self, _line: &str) {}
}

/// A clock moved by the test and by `pause`; its waits are short real sleeps so a watcher notices the new time soon.
struct FakeClock(Arc<World>);

impl Clock for FakeClock {
    /// The scripted time.
    fn now(&self) -> Duration {
        Duration::from_millis(self.0.now_ms.load(Ordering::SeqCst))
    }

    /// Sleeps 1 ms and moves the scripted time by ten times `d`, so a cold start that never finishes reaches its limit in a fraction of a second while a test can still watch it.
    fn pause(&self, d: Duration) {
        std::thread::sleep(Duration::from_millis(1));
        self.0.now_ms.fetch_add(d.as_millis() as u64 * 10, Ordering::SeqCst);
    }

    /// A 2 ms real wait.
    fn wait<'a, T>(&self, cv: &Condvar, guard: MutexGuard<'a, T>, _d: Duration) -> MutexGuard<'a, T> {
        cv.wait_timeout(guard, Duration::from_millis(2)).unwrap().0
    }
}

type Rig = (Arc<World>, Arc<Engine<FakeHost, FakeClock>>);

/// A fresh world with idle time `idle` seconds and an engine over it.
fn rig(idle: i64) -> Rig {
    let world = Arc::new(World::default());
    world.idle.store(idle, Ordering::SeqCst);
    let engine = Engine::new(FakeHost(Arc::clone(&world)), FakeClock(Arc::clone(&world)));
    (world, engine)
}

/// Moves the scripted clock forward by `secs`.
fn advance(world: &World, secs: u64) {
    world.now_ms.fetch_add(secs * 1000, Ordering::SeqCst);
}

/// Waits until `check` holds, up to five seconds.
fn eventually(check: impl Fn() -> bool) {
    let until = Instant::now() + Duration::from_secs(5);
    while !check() {
        assert!(Instant::now() < until, "condition never held");
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// A second request reuses the running server instead of starting another.
#[test]
fn a_running_server_is_reused() {
    let (world, engine) = rig(120);
    let first = engine.acquire(MODEL, &Cancel::never()).unwrap();
    let second = engine.acquire(MODEL, &Cancel::never()).unwrap();
    assert_eq!(world.launches.load(Ordering::SeqCst), 1);
    assert_eq!(first.conn, second.conn);
}

/// After the idle time with nothing holding it the one watcher stops the server, and the state returns to stopped with no watcher.
#[test]
fn an_idle_server_is_stopped_by_its_watcher() {
    let (world, engine) = rig(10);
    drop(engine.acquire(MODEL, &Cancel::never()).unwrap());
    assert_eq!(engine.snapshot().phase, "running");
    advance(&world, 11);
    eventually(|| world.alive.load(Ordering::SeqCst) == 0);
    eventually(|| {
        let now = engine.snapshot();
        now.phase == "stopped" && now.watcher.is_none()
    });
}

/// A held lease keeps the server across the idle time; releasing it restarts the clock, and the server stops one idle time later.
#[test]
fn a_lease_held_across_idle_keeps_the_server() {
    let (world, engine) = rig(10);
    let lease = engine.acquire(MODEL, &Cancel::never()).unwrap();
    advance(&world, 60);
    std::thread::sleep(Duration::from_millis(30));
    assert_eq!(world.alive.load(Ordering::SeqCst), 1);
    drop(lease);
    std::thread::sleep(Duration::from_millis(30));
    assert_eq!(world.alive.load(Ordering::SeqCst), 1);
    advance(&world, 11);
    eventually(|| world.alive.load(Ordering::SeqCst) == 0);
}

/// Idle time 0 stops the server the moment the last lease is dropped, and not before.
#[test]
fn idle_zero_stops_at_the_last_release() {
    let (world, engine) = rig(0);
    let a = engine.acquire(MODEL, &Cancel::never()).unwrap();
    let b = engine.acquire(MODEL, &Cancel::never()).unwrap();
    drop(a);
    assert_eq!(world.alive.load(Ordering::SeqCst), 1);
    drop(b);
    assert_eq!(world.alive.load(Ordering::SeqCst), 0);
    assert_eq!(engine.snapshot().phase, "stopped");
    assert!(!engine.keeps_warm());
}

/// Idle time below 0 means never: no watcher is started and the server outlives any amount of idle time.
#[test]
fn idle_never_starts_no_watcher_and_keeps_the_server() {
    let (world, engine) = rig(-1);
    drop(engine.acquire(MODEL, &Cancel::never()).unwrap());
    advance(&world, 100_000);
    std::thread::sleep(Duration::from_millis(30));
    assert_eq!(engine.snapshot().phase, "running");
    assert_eq!(engine.snapshot().watcher, None);
    assert_eq!(world.alive.load(Ordering::SeqCst), 1);
}

/// A start that fails leaves the engine stopped and the next call starts normally.
#[test]
fn a_failed_start_leaves_the_engine_usable() {
    let (world, engine) = rig(120);
    world.fail_launch.store(true, Ordering::SeqCst);
    assert_eq!(engine.acquire(MODEL, &Cancel::never()).err(), Some(Fail::Failed("no process".into())));
    assert_eq!(engine.snapshot().phase, "stopped");
    world.fail_launch.store(false, Ordering::SeqCst);
    assert!(engine.acquire(MODEL, &Cancel::never()).is_ok());
    assert_eq!(world.alive.load(Ordering::SeqCst), 1);
}

/// When the first start fails and the host allows a retry, the safe start is tried once, remembered and used.
#[test]
fn a_failed_start_is_retried_the_safe_way() {
    let (world, engine) = rig(120);
    world.fail_first_probe.store(true, Ordering::SeqCst);
    world.retry_allowed.store(true, Ordering::SeqCst);
    assert!(engine.acquire(MODEL, &Cancel::never()).is_ok());
    assert_eq!(world.launches.load(Ordering::SeqCst), 2);
    assert_eq!(world.kills.load(Ordering::SeqCst), 1);
    assert!(world.fallback_recorded.load(Ordering::SeqCst));
}

/// The race behind the orphan server: the idle stop is killing the old process while a new request arrives. The request waits for the stop, starts one replacement, and the replacement gets its own watcher.
#[test]
fn a_request_during_an_idle_stop_gets_one_replacement_with_its_own_watcher() {
    let (world, engine) = rig(10);
    drop(engine.acquire(MODEL, &Cancel::never()).unwrap());
    let old_generation = engine.snapshot().gen;
    let gate = Arc::new(Barrier::new(2));
    *world.kill_gate.lock().unwrap() = Some(Arc::clone(&gate));
    advance(&world, 11);
    gate.wait();
    assert_eq!(engine.snapshot().phase, "stopping");
    let waiting = {
        let engine = Arc::clone(&engine);
        std::thread::spawn(move || engine.acquire(MODEL, &Cancel::never()).map(|lease| lease.conn.port))
    };
    std::thread::sleep(Duration::from_millis(60));
    assert_eq!(world.launches.load(Ordering::SeqCst), 1, "no replacement while the old process is being killed");
    *world.kill_gate.lock().unwrap() = None;
    gate.wait();
    assert_eq!(waiting.join().unwrap(), Ok(1));
    assert_eq!(world.launches.load(Ordering::SeqCst), 2);
    assert_eq!(world.alive.load(Ordering::SeqCst), 1);
    let now = engine.snapshot();
    assert_eq!(now.phase, "running");
    assert_ne!(now.gen, old_generation);
    assert_eq!(now.watcher, Some(now.gen));
}

/// While a model is being deleted new calls are refused, the running server is stopped first, and calls work again afterwards.
#[test]
fn deleting_the_model_excludes_new_calls() {
    let (world, engine) = rig(120);
    drop(engine.acquire(MODEL, &Cancel::never()).unwrap());
    let guard = engine.stop_for_delete();
    assert_eq!(world.alive.load(Ordering::SeqCst), 0);
    assert!(matches!(engine.acquire(MODEL, &Cancel::never()), Err(Fail::Failed(_))));
    drop(guard);
    assert!(engine.acquire(MODEL, &Cancel::never()).is_ok());
}

/// A delete waits for a held lease to finish before it stops the server.
#[test]
fn a_delete_waits_for_a_held_lease() {
    let (world, engine) = rig(120);
    let lease = engine.acquire(MODEL, &Cancel::never()).unwrap();
    let deleting = {
        let engine = Arc::clone(&engine);
        std::thread::spawn(move || drop(engine.stop_for_delete()))
    };
    std::thread::sleep(Duration::from_millis(40));
    assert_eq!(world.alive.load(Ordering::SeqCst), 1);
    drop(lease);
    deleting.join().unwrap();
    assert_eq!(world.alive.load(Ordering::SeqCst), 0);
}

/// Aborting a lease ends its server at once even though it is held, and a lease from a replaced server cannot stop its successor.
#[test]
fn an_aborted_lease_ends_its_server_and_a_stale_one_changes_nothing() {
    let (world, engine) = rig(120);
    let old = engine.acquire(MODEL, &Cancel::never()).unwrap();
    old.aborter().abort();
    assert_eq!(world.alive.load(Ordering::SeqCst), 0);
    let fresh = engine.acquire(MODEL, &Cancel::never()).unwrap();
    old.aborter().abort();
    drop(old);
    assert_eq!(world.alive.load(Ordering::SeqCst), 1);
    drop(fresh);
}

/// A cancel that fires while the model is loading kills the starting process, reports Cancelled, leaves the engine stopped and lets the next call start.
#[test]
fn cancelling_a_cold_start_kills_it_and_the_next_call_starts() {
    let (world, engine) = rig(120);
    world.loading_polls.store(1_000_000, Ordering::SeqCst);
    let flag = Arc::new(AtomicBool::new(false));
    let cancel = {
        let flag = Arc::clone(&flag);
        Cancel::new(move || flag.load(Ordering::SeqCst))
    };
    let starting = {
        let engine = Arc::clone(&engine);
        std::thread::spawn(move || engine.acquire(MODEL, &cancel).err())
    };
    eventually(|| engine.snapshot().phase == "starting");
    flag.store(true, Ordering::SeqCst);
    assert_eq!(starting.join().unwrap(), Some(Fail::Cancelled));
    assert_eq!(world.alive.load(Ordering::SeqCst), 0);
    assert_eq!(engine.snapshot().phase, "stopped");
    world.loading_polls.store(0, Ordering::SeqCst);
    assert!(engine.acquire(MODEL, &Cancel::never()).is_ok());
}

/// A call already cancelled when it arrives starts nothing; a model that never loads ends in a failure, not a hang.
#[test]
fn a_cancelled_call_starts_nothing_and_a_model_that_never_loads_fails() {
    let (world, engine) = rig(120);
    assert_eq!(engine.acquire(MODEL, &Cancel::new(|| true)).err(), Some(Fail::Cancelled));
    assert_eq!(world.launches.load(Ordering::SeqCst), 0);
    world.loading_polls.store(usize::MAX, Ordering::SeqCst);
    assert!(matches!(engine.acquire(MODEL, &Cancel::never()), Err(Fail::Failed(_))));
    assert_eq!(world.alive.load(Ordering::SeqCst), 0);
}

/// A panic while the model loads kills the process that was launched, leaves the engine stopped and usable.
#[test]
fn a_panic_during_a_start_kills_the_launched_process() {
    let (world, engine) = rig(120);
    world.panic_on_probe.store(true, Ordering::SeqCst);
    assert!(matches!(engine.acquire(MODEL, &Cancel::never()), Err(Fail::Failed(_))));
    assert_eq!(world.alive.load(Ordering::SeqCst), 0);
    assert_eq!(engine.snapshot().phase, "stopped");
    world.panic_on_probe.store(false, Ordering::SeqCst);
    assert!(engine.acquire(MODEL, &Cancel::never()).is_ok());
}

/// When the idle watcher thread cannot start, the server still works, no watcher is recorded, and a later lease tries again.
#[test]
fn a_watcher_that_cannot_start_is_recorded_and_retried() {
    let (world, engine) = rig(10);
    world.spawn_fails.store(true, Ordering::SeqCst);
    let lease = engine.acquire(MODEL, &Cancel::never()).unwrap();
    assert_eq!(engine.snapshot().watcher, None);
    world.spawn_fails.store(false, Ordering::SeqCst);
    let second = engine.acquire(MODEL, &Cancel::never()).unwrap();
    let now = engine.snapshot();
    assert_eq!(now.watcher, Some(now.gen));
    drop((lease, second));
}

/// A request for another model replaces the idle server of the first; while the first is held the request waits instead.
#[test]
fn another_model_replaces_an_idle_server_and_waits_for_a_held_one() {
    let (world, engine) = rig(120);
    let held = engine.acquire(MODEL, &Cancel::never()).unwrap();
    let cancelled = Cancel::new({
        let started = Instant::now();
        move || started.elapsed() > Duration::from_millis(60)
    });
    assert_eq!(engine.acquire("model-b", &cancelled).err(), Some(Fail::Cancelled));
    assert_eq!(world.launches.load(Ordering::SeqCst), 1);
    drop(held);
    assert!(engine.acquire("model-b", &Cancel::never()).is_ok());
    assert_eq!(world.launches.load(Ordering::SeqCst), 2);
    assert_eq!(world.alive.load(Ordering::SeqCst), 1);
}

/// A server whose process died is replaced even though a lease still holds it, and the old lease cannot disturb the new one.
#[test]
fn a_dead_server_is_replaced_even_when_held() {
    let (world, engine) = rig(120);
    let old = engine.acquire(MODEL, &Cancel::never()).unwrap();
    world.dead.store(true, Ordering::SeqCst);
    let probe_thread = {
        let engine = Arc::clone(&engine);
        std::thread::spawn(move || engine.acquire(MODEL, &Cancel::never()).map(|lease| lease.conn.port))
    };
    eventually(|| world.launches.load(Ordering::SeqCst) == 2);
    world.dead.store(false, Ordering::SeqCst);
    assert_eq!(probe_thread.join().unwrap(), Ok(1));
    drop(old);
    assert_eq!(world.alive.load(Ordering::SeqCst), 1);
}

/// Retiring while stopped or while a start is in progress changes nothing: no phase is left stuck in Stopping.
#[test]
fn retiring_outside_the_running_phase_leaves_the_phase_alone() {
    let (world, engine) = rig(120);
    engine.retire_in_any_phase();
    assert_eq!(engine.snapshot().phase, "stopped");
    world.loading_polls.store(1_000_000, Ordering::SeqCst);
    let flag = Arc::new(AtomicBool::new(false));
    let cancel = {
        let flag = Arc::clone(&flag);
        Cancel::new(move || flag.load(Ordering::SeqCst))
    };
    let starting = {
        let engine = Arc::clone(&engine);
        std::thread::spawn(move || engine.acquire(MODEL, &cancel).err())
    };
    eventually(|| engine.snapshot().phase == "starting");
    engine.retire_in_any_phase();
    assert_eq!(engine.snapshot().phase, "starting");
    flag.store(true, Ordering::SeqCst);
    assert_eq!(starting.join().unwrap(), Some(Fail::Cancelled));
    assert_eq!(engine.snapshot().phase, "stopped");
}
