//! File: input/view.rs
//! Purpose: What the settings window sees of input: revisioned snapshots, each carrying a revision or sequence number so the window can see a gap and ask for a fresh one. Tests also subscribe to a hub to read every published value in order.
//! Contents: Hub — holds and publishes the latest value (in tests, to subscribers), and hands out each change once (`take_dirty`); Subscription / Update (tests only) — one subscriber's stream; CatalogView / ShortcutView — the shortcut list; RecorderView / Phase / Event / Noted — one recording and its transient events; Completion — a one-shot answer.

#[cfg(test)]
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
#[cfg(test)]
use std::sync::mpsc::{SyncSender, TryRecvError, TrySendError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;

use super::chord::Part;

#[cfg(test)]
const QUEUE: usize = 64;

#[cfg(test)]
struct Sub<T> {
    tx: SyncSender<T>,
    gap: Arc<AtomicBool>,
}

struct Inner<T> {
    latest: T,
    version: u64,
    emitted: u64,
    #[cfg(test)]
    subs: Vec<Sub<T>>,
}

/// Holds the latest value and tells every subscriber about each change.
pub struct Hub<T: Clone> {
    inner: Mutex<Inner<T>>,
}

impl<T: Clone> Hub<T> {
    /// A hub whose first snapshot is `first`.
    pub fn new(first: T) -> Arc<Hub<T>> {
        Arc::new(Hub {
            inner: Mutex::new(Inner {
                latest: first,
                version: 0,
                emitted: 0,
                #[cfg(test)]
                subs: Vec::new(),
            }),
        })
    }

    /// Makes `value` the latest and, in tests, sends it to every subscriber; one that cannot keep up is marked and will be given a fresh snapshot.
    pub fn publish(&self, value: T) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.latest = value.clone();
        inner.version += 1;
        #[cfg(test)]
        inner.subs.retain(|s| match s.tx.try_send(value.clone()) {
            Err(TrySendError::Full(_)) => {
                s.gap.store(true, Ordering::Release);
                true
            }
            Err(TrySendError::Disconnected(_)) => false,
            Ok(()) => true,
        });
    }

    /// The latest value when it is newer than the last one handed out here, marking it handed out; None when nothing changed since. The effects side emits through this, so each change is emitted at least once and an unchanged hub is not emitted again.
    pub fn take_dirty(&self) -> Option<T> {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        (inner.version > inner.emitted).then(|| {
            inner.emitted = inner.version;
            inner.latest.clone()
        })
    }

    /// Whether the latest value is newer than the last one handed out.
    pub fn is_dirty(&self) -> bool {
        let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.version > inner.emitted
    }

    /// The latest value.
    pub fn latest(&self) -> T {
        self.inner.lock().unwrap_or_else(|e| e.into_inner()).latest.clone()
    }

    /// A subscription whose first update is the latest value.
    #[cfg(test)]
    pub fn subscribe(self: &Arc<Hub<T>>) -> Subscription<T> {
        let (tx, rx) = mpsc::sync_channel(QUEUE);
        let gap = Arc::new(AtomicBool::new(false));
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let _ = tx.try_send(inner.latest.clone());
        inner.subs.push(Sub { tx, gap: gap.clone() });
        Subscription { rx, gap, hub: self.clone() }
    }
}

/// One item of a subscription: the next value, or a fresh snapshot because some were missed.
#[cfg(test)]
#[derive(Debug, PartialEq)]
pub enum Update<T> {
    Next(T),
    Resnapshot(T),
}

/// An ordered stream of a hub's values: a snapshot first, then each change.
#[cfg(test)]
pub struct Subscription<T: Clone> {
    rx: Receiver<T>,
    gap: Arc<AtomicBool>,
    hub: Arc<Hub<T>>,
}

#[cfg(test)]
impl<T: Clone> Subscription<T> {
    /// The next update if one is ready.
    pub fn try_next(&self) -> Option<Update<T>> {
        if self.gap.swap(false, Ordering::AcqRel) {
            while self.rx.try_recv().is_ok() {}
            return Some(Update::Resnapshot(self.hub.latest()));
        }
        match self.rx.try_recv() {
            Ok(v) => Some(Update::Next(v)),
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => None,
        }
    }
}

/// Where a recording is in its life.
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Idle,
    Starting,
    Armed,
}

/// Why a recording ended, or what just happened in it.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Event {
    None,
    Saved,
    Invalid { message: String },
    Cancelled { reason: Reason },
    Failed { message: String },
}

/// Why a recording was cancelled.
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    User,
    Outside,
    Expired,
    Replaced,
    WindowClosed,
    Interrupted,
    Reset,
    Revoked,
}

/// An event stamped with the view `seq` that carried it, so a window that missed views still sees it once.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Noted {
    pub seq: u64,
    pub event: Event,
}

/// shortcut: a window that misses more than `NOTED` events loses the oldest, upgrade to a cumulative counter with an overflow flag if the effects thread can stall through that many user events.
/// How many transient events a view keeps for a window that missed the views that carried them.
const NOTED: usize = 8;

/// Everything the window shows of one recording; `seq` grows with every change, `lease` names the recording; `events` keeps the last transient events (rejection, saved, end) with the seq that carried each, so coalescing views never loses one.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct RecorderView {
    pub lease: u64,
    pub seq: u64,
    pub phase: Phase,
    pub open: bool,
    pub parts: Vec<Part>,
    pub live: bool,
    pub held: bool,
    pub error: Option<String>,
    pub saves: u32,
    pub revision: u64,
    pub event: Event,
    pub events: Vec<Noted>,
}

impl RecorderView {
    /// Adds the view's current event to `events` under the view's `seq` (nothing for `Event::None`), keeping the last `NOTED`.
    pub fn note(&mut self) {
        if self.event == Event::None {
            return;
        }
        self.events.push(Noted { seq: self.seq, event: self.event.clone() });
        let extra = self.events.len().saturating_sub(NOTED);
        self.events.drain(..extra);
    }
}

/// One shortcut in the list: its keys, whether it is live, whether its action is held now, and a problem to show.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct ShortcutView {
    pub id: &'static str,
    pub label: &'static str,
    pub parts: Vec<Part>,
    pub enabled: bool,
    pub held: bool,
    pub error: Option<String>,
}

/// The list of shortcuts at one revision, with the health of the keyboard hook and the last late save failure.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct CatalogView {
    pub revision: u64,
    pub shortcuts: Vec<ShortcutView>,
    pub health: super::health::Health,
    pub last_error: Option<String>,
}

/// A one-shot answer to a request, ready when the input thread has handled it.
pub struct Completion<T>(Receiver<T>);

impl<T> Completion<T> {
    /// A completion and the sender that fulfils it.
    pub fn pair() -> (mpsc::Sender<T>, Completion<T>) {
        let (tx, rx) = mpsc::channel();
        (tx, Completion(rx))
    }

    /// The answer if it is ready.
    #[cfg(test)]
    pub fn try_get(&self) -> Option<T> {
        self.0.try_recv().ok()
    }

    /// The answer, waiting up to `wait`.
    pub fn wait(&self, wait: Duration) -> Option<T> {
        self.0.recv_timeout(wait).ok()
    }
}
