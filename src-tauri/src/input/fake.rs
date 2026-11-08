//! File: input/fake.rs
//! Purpose: A scripted keyboard for tests: the same reducer as the Windows adapter, a virtual clock, scripted key edges and a store that can block or fail. It records what the foreground app would have received and which actions are active, so tests assert pass or swallow per edge, one terminal per recording and the downstream key log.
//! Contents: Fake — the keyboard, the clock and the `Input` under test (the store, the log and the sink are in `fake_parts`).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::{Duration, Instant};

use super::api::Input;
use super::catalog::Catalog;
use super::commands::Command;
use super::core::{Core, Edge, Origin, Verdict};
use super::driver::{ArmOutcome, Runtime};
use super::effects::Gate;
use super::fake_parts::TestSink;
use super::key::{Key, Mods};
use super::ledger::Slot;
use super::mailbox::Mailbox;
use super::popup::Rect;
use super::shared::Shared;

pub use super::fake_parts::{FakeStore, Log};

/// The scripted keyboard and the `Input` it drives.
pub struct Fake {
    pub input: Input,
    pub store: Arc<FakeStore>,
    pub log: Arc<Mutex<Log>>,
    pub downstream: Mutex<Vec<String>>,
    pub hold_arm: Arc<AtomicBool>,
    pub auto_pump: Arc<AtomicBool>,
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
        let hold_arm = Arc::new(AtomicBool::new(false));
        let holding = hold_arm.clone();
        let auto_pump = Arc::new(AtomicBool::new(true));
        let mailbox = Arc::new(Mailbox::new(move || {
            if let Some(rt) = waker.get().and_then(Weak::upgrade) {
                let (base, offset) = *ticking.lock().unwrap();
                let mut rt = rt.lock().unwrap();
                rt.process(base + offset);
                if rt.recovering() && !holding.load(Ordering::Acquire) {
                    rt.complete_recover(Ok(()));
                }
            }
        }));
        let shared = Arc::new(Shared::default());
        let core = Core::new(catalog, store.clone(), mailbox.clone(), shared.clone());
        let (hub, gens) = (core.catalog_hub(), core.gens());
        let log = Arc::new(Mutex::new(Log::default()));
        let sink = TestSink { log: log.clone(), gate: Gate::new(gens, catalog.specs.len()) };
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
        Fake { input, store, log, downstream: Mutex::new(Vec::new()), hold_arm, auto_pump, rt, clock }
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
        let mut rt = self.rt.lock().unwrap();
        rt.tick(now);
        rt.pump_writes(now);
    }

    /// Sends one edge from the scripted keyboard; what passes is added to the downstream log.
    fn edge(&self, slot: Slot, down: bool, origin: Origin) -> Verdict {
        let now = self.now();
        let verdict = {
            let mut rt = self.rt.lock().unwrap();
            let verdict = rt.edge(Edge { slot, down, origin }, now);
            if self.auto_pump.load(Ordering::Acquire) {
                rt.pump_writes(now);
            }
            verdict
        };
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
        let mut rt = self.rt.lock().unwrap();
        rt.pointer_down(x, y, now);
        rt.pump_writes(now);
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
        let mut rt = self.rt.lock().unwrap();
        rt.process(now);
        if rt.recovering() && !self.hold_arm.load(Ordering::Acquire) {
            rt.complete_recover(Ok(()));
        }
        rt.pump_writes(now);
    }

    /// The nearest deadline of the running recording or popup.
    pub fn next_deadline(&self) -> Option<Instant> {
        self.rt.lock().unwrap().core.next_deadline()
    }

    /// Whether a queued write waits to be handed to the writer.
    pub fn writes_due(&self) -> bool {
        self.rt.lock().unwrap().core.writes_due()
    }

    /// What the message loop does after a callback: hands the next queued write to the writer.
    pub fn drive(&self) {
        let now = self.now();
        self.rt.lock().unwrap().pump_writes(now);
    }

    /// Ends the surfaces as the driver does when a part they need could not be made.
    pub fn fail_surfaces(&self, message: &str) {
        let now = self.now();
        self.rt.lock().unwrap().fail_surfaces(message.into(), now);
    }

    /// Answers a recovery that waited for the hook to be made again.
    pub fn finish_recover(&self) {
        self.rt.lock().unwrap().complete_recover(Ok(()));
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
