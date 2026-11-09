//! File: input/fake_parts.rs
//! Purpose: The pieces the scripted keyboard is built from: a store in memory whose writes can be held back or made to fail, the log of what the effects side did, and the sink that runs the same gate as the real effects thread.
//! Contents: FakeStore — the store; Log — what the sink saw; TestSink — the sink.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};

use super::chord::Chord;
use super::dirty::Dirty;
use super::effects::{Effect, Gate, Sink};
use super::save::Store;

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
    pub reports: Vec<String>,
    pub emitted: Vec<(&'static str, serde_json::Value)>,
    pub slow: bool,
    pub mask_refuses: bool,
    pub mask_failures: u32,
}

/// The test sink: runs the same gate as the real effects thread.
pub struct TestSink {
    pub log: Arc<Mutex<Log>>,
    pub gate: Gate,
    pub dirty: Arc<Dirty>,
}

impl TestSink {
    /// Emits the changed views into the log, as the effects thread does each time it wakes (not while the test makes the thread slow: views wait until a `wake`).
    fn drain(&self, log: &mut Log) {
        if log.slow {
            return;
        }
        self.dirty.drain(|event, value| log.emitted.push((event, value)));
    }
}

impl Sink for TestSink {
    /// Carries the effect out, or refuses it when the test says the queue is full.
    fn send(&mut self, effect: Effect) -> bool {
        let mut log = self.log.lock().unwrap();
        if log.refuse && matches!(effect, Effect::Action { .. } | Effect::Escape | Effect::Typing | Effect::Views) {
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
            Effect::Check => {}
            Effect::Report(message) => log.reports.push(message),
            Effect::Escape => log.escapes += 1,
            Effect::Typing => log.typing += 1,
            Effect::Views => {}
        }
        self.drain(&mut log);
        true
    }

    /// Runs the releases that were refused and reports a mask failure, and (unless the queue is refusing) emits the changed views, as the real effects thread does when woken.
    fn wake(&mut self) {
        let mut log = self.log.lock().unwrap();
        log.slow = false;
        log.mask_failures += u32::from(self.gate.take_mask_failure());
        for idx in self.gate.owed_releases() {
            log.actions.push((idx as u8, false));
            log.active.remove(&(idx as u8));
        }
        if !log.refuse {
            self.drain(&mut log);
        }
    }
}
