//! File: input/fake.rs
//! Purpose: A scripted keyboard for tests: the same reducer as the Windows adapter, a virtual clock, scripted key edges and a store that can block or fail. It records what the foreground app would have received and which actions are active, so tests assert pass or swallow per edge, one terminal per recording and the downstream key log.
//! Contents: FakeStore — a store with a gate and a failure switch; Log — what the sink saw; Fake — the keyboard, the clock and the `Input` under test.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock, Weak};
use std::time::{Duration, Instant};

use super::api::Input;
use super::catalog::Catalog;
use super::chord::Chord;
use super::commands::Command;
use super::core::{Core, Edge, Origin, Verdict};
use super::driver::{ArmOutcome, Runtime};
use super::effects::{Effect, Gate, Sink};
use super::key::{Key, Mods};
use super::ledger::Slot;
use super::mailbox::Mailbox;
use super::popup::Rect;
use super::save::Store;
use super::shared::Shared;

/// A store in memory whose writes can be held back or made to fail.
pub struct FakeStore {
    values: Mutex<HashMap<String, serde_json::Value>>,
    off: Mutex<HashSet<String>>,
    blocked: Mutex<bool>,
    gate: Condvar,
    pub fail: AtomicBool,
    pub writes: AtomicUsize,
}

impl FakeStore {
    /// An empty store: every shortcut on, nothing saved.
    pub fn new() -> Arc<FakeStore> {
        Arc::new(FakeStore {
            values: Mutex::new(HashMap::new()),
            off: Mutex::new(HashSet::new()),
            blocked: Mutex::new(false),
            gate: Condvar::new(),
            fail: AtomicBool::new(false),
            writes: AtomicUsize::new(0),
        })
    }

    /// Puts a raw stored value, as an older build would have written it.
    pub fn put(&self, id: &str, value: serde_json::Value) {
        self.values.lock().unwrap().insert(id.into(), value);
    }

    /// The stored value.
    pub fn get(&self, id: &str) -> Option<serde_json::Value> {
        self.values.lock().unwrap().get(id).cloned()
    }

    /// Holds every write until `unblock`.
    pub fn block(&self) {
        *self.blocked.lock().unwrap() = true;
    }

    /// Lets held writes through.
    pub fn unblock(&self) {
        *self.blocked.lock().unwrap() = false;
        self.gate.notify_all();
    }

    /// How many writes reached the store.
    pub fn writes_len(&self) -> usize {
        self.writes.load(Ordering::Acquire)
    }

    /// Turns a shortcut or its feature on or off.
    pub fn set_enabled(&self, id: &str, on: bool) {
        let mut off = self.off.lock().unwrap();
        if on {
            off.remove(id)
        } else {
            off.insert(id.into())
        };
    }
}

impl Store for FakeStore {
    /// The stored value.
    fn load(&self, id: &str) -> Option<serde_json::Value> {
        self.get(id)
    }

    /// Waits while blocked, then stores or fails.
    fn save(&self, id: &str, chord: &Chord) -> Result<(), String> {
        let mut blocked = self.blocked.lock().unwrap();
        while *blocked {
            blocked = self.gate.wait(blocked).unwrap();
        }
        self.writes.fetch_add(1, Ordering::AcqRel);
        if self.fail.load(Ordering::Acquire) {
            return Err("disk full".into());
        }
        self.values.lock().unwrap().insert(id.into(), serde_json::to_value(chord).unwrap());
        Ok(())
    }

    /// Whether the shortcut is on.
    fn enabled(&self, id: &str) -> bool {
        !self.off.lock().unwrap().contains(id)
    }
}

/// What the effects side did.
#[derive(Default)]
pub struct Log {
    pub actions: Vec<(u8, bool)>,
    pub active: BTreeSet<u8>,
    pub escapes: u32,
    pub typing: u32,
    pub masks: u32,
    pub refuse: bool,
}

/// The test sink: runs the same gate as the real effects thread.
pub struct TestSink {
    log: Arc<Mutex<Log>>,
    gate: Gate,
}

impl Sink for TestSink {
    /// Carries the effect out, or refuses it when the test says the queue is full.
    fn send(&mut self, effect: Effect) -> bool {
        let mut log = self.log.lock().unwrap();
        if log.refuse && matches!(effect, Effect::Action { .. } | Effect::Escape | Effect::Typing) {
            return false;
        }
        match effect {
            Effect::Action { idx, down, gen } => {
                if self.gate.admit(idx as usize, down, gen) {
                    log.actions.push((idx, down));
                    if down {
                        log.active.insert(idx)
                    } else {
                        log.active.remove(&idx)
                    };
                }
            }
            Effect::Escape => log.escapes += 1,
            Effect::Typing => log.typing += 1,
            Effect::MenuMask => log.masks += 1,
            Effect::Recorder(hub, view) => hub.publish(view),
            Effect::Catalog(hub, view) => hub.publish(view),
        }
        true
    }
}

/// The scripted keyboard and the `Input` it drives.
pub struct Fake {
    pub input: Input,
    pub store: Arc<FakeStore>,
    pub log: Arc<Mutex<Log>>,
    pub downstream: Mutex<Vec<String>>,
    pub hold_arm: Arc<AtomicBool>,
    rt: Arc<Mutex<Runtime<TestSink>>>,
    clock: Arc<Mutex<(Instant, Duration)>>,
}

impl Fake {
    /// A keyboard over `catalog` and an empty store.
    pub fn new(catalog: &'static Catalog) -> Fake {
        Fake::with_store(catalog, FakeStore::new())
    }

    /// A keyboard over `catalog` and `store`.
    pub fn with_store(catalog: &'static Catalog, store: Arc<FakeStore>) -> Fake {
        let slot: Arc<OnceLock<Weak<Mutex<Runtime<TestSink>>>>> = Arc::new(OnceLock::new());
        let waker = slot.clone();
        let clock = Arc::new(Mutex::new((Instant::now(), Duration::ZERO)));
        let ticking = clock.clone();
        let mailbox = Arc::new(Mailbox::new(move || {
            if let Some(rt) = waker.get().and_then(Weak::upgrade) {
                let (base, offset) = *ticking.lock().unwrap();
                rt.lock().unwrap().process(base + offset);
            }
        }));
        let shared = Arc::new(Shared::default());
        let core = Core::new(catalog, store.clone(), mailbox.clone(), shared.clone());
        let (hub, gens) = (core.catalog_hub(), core.gens());
        let log = Arc::new(Mutex::new(Log::default()));
        let sink = TestSink { log: log.clone(), gate: Gate::new(gens, catalog.specs.len()) };
        let hold_arm = Arc::new(AtomicBool::new(false));
        let hold = hold_arm.clone();
        let rt = Arc::new(Mutex::new(Runtime::new(core, sink, mailbox.clone(), move || {
            if hold.load(Ordering::Acquire) {
                ArmOutcome::Later
            } else {
                ArmOutcome::Ready
            }
        })));
        let _ = slot.set(Arc::downgrade(&rt));
        let input = Input::new(mailbox, hub, shared, catalog, Box::new(|| {}));
        rt.lock().unwrap().process(clock.lock().unwrap().0);
        Fake { input, store, log, downstream: Mutex::new(Vec::new()), hold_arm, rt, clock }
    }

    /// The virtual time now.
    pub fn now(&self) -> Instant {
        let c = self.clock.lock().unwrap();
        c.0 + c.1
    }

    /// Moves the virtual clock and runs the deadlines that passed.
    pub fn advance(&self, by: Duration) {
        self.clock.lock().unwrap().1 += by;
        let now = self.now();
        self.rt.lock().unwrap().tick(now);
    }

    /// Sends one edge from the scripted keyboard; what passes is added to the downstream log.
    fn edge(&self, slot: Slot, down: bool, origin: Origin) -> Verdict {
        let now = self.now();
        let verdict = self.rt.lock().unwrap().edge(Edge { slot, down, origin }, now);
        if verdict == Verdict::Pass && origin != Origin::Phantom {
            let name = match slot {
                Slot::Key(k) => k.name().to_string(),
                Slot::Mod(m) => format!("mod{}", m.0),
            };
            self.downstream.lock().unwrap().push(format!("{}{name}", if down { '+' } else { '-' }));
        }
        verdict
    }

    /// A physical key down (a second one without an up is a repeat).
    pub fn down(&self, key: Key) -> Verdict {
        self.edge(Slot::Key(key), true, Origin::Physical)
    }

    /// A physical key up.
    pub fn up(&self, key: Key) -> Verdict {
        self.edge(Slot::Key(key), false, Origin::Physical)
    }

    /// A physical modifier down.
    pub fn mod_down(&self, m: Mods) -> Verdict {
        self.edge(Slot::Mod(m), true, Origin::Physical)
    }

    /// A physical modifier up.
    pub fn mod_up(&self, m: Mods) -> Verdict {
        self.edge(Slot::Mod(m), false, Origin::Physical)
    }

    /// An edge with an explicit origin (Cue's own key, an AltGr phantom).
    pub fn edge_from(&self, slot: Slot, down: bool, origin: Origin) -> Verdict {
        self.edge(slot, down, origin)
    }

    /// Presses and releases a key.
    pub fn tap(&self, key: Key) {
        self.down(key);
        self.up(key);
    }

    /// A pointer button down at (x, y).
    pub fn pointer(&self, x: i32, y: i32) {
        let now = self.now();
        self.rt.lock().unwrap().pointer_down(x, y, now);
    }

    /// Answers a held-back arm request.
    pub fn arm_now(&self) {
        let now = self.now();
        self.rt.lock().unwrap().complete_arm(ArmOutcome::Ready, now);
    }

    /// A lock, resume or desktop change.
    pub fn session(&self) {
        let now = self.now();
        self.rt.lock().unwrap().session(now);
    }

    /// Handles queued commands (a late save result, for one).
    pub fn pump(&self) {
        let now = self.now();
        self.rt.lock().unwrap().process(now);
    }

    /// Waits until the writer has answered `n` writes and the answers are handled.
    pub fn settle(&self, n: usize) {
        for _ in 0..2000 {
            if self.store.writes.load(Ordering::Acquire) >= n {
                std::thread::sleep(Duration::from_millis(20));
                self.pump();
                return;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        panic!("the writer did not run {n} writes");
    }

    /// Posts a command straight to the input thread, for requests no handle can produce.
    pub fn post(&self, command: Command) {
        self.input.inner.mailbox.post(command);
    }

    /// The downstream key log so far.
    pub fn passed(&self) -> Vec<String> {
        self.downstream.lock().unwrap().clone()
    }

    /// The actions currently active.
    pub fn active(&self) -> Vec<u8> {
        self.log.lock().unwrap().active.iter().copied().collect()
    }

    /// Makes the sink refuse the next effects, as a full queue would.
    pub fn refuse(&self, on: bool) {
        self.log.lock().unwrap().refuse = on;
    }

    /// A rectangle for outside-click tests.
    pub fn rect(left: i32, top: i32, right: i32, bottom: i32) -> Rect {
        Rect { left, top, right, bottom }
    }
}
