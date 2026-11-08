//! File: input/view.rs
//! Purpose: What the settings window sees of input: revisioned snapshots and the subscription that delivers them in order, with a fresh snapshot after a gap.
//! Contents: Hub — publishes the latest value to subscribers; Subscription / Update — one subscriber's stream; CatalogView / ShortcutView — the shortcut list; RecorderView / Phase / Event — one recording; Completion — a one-shot answer.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;

use super::chord::Part;

const QUEUE: usize = 64;

struct Sub<T> {
    tx: SyncSender<T>,
    gap: Arc<AtomicBool>,
}

struct Inner<T> {
    latest: T,
    subs: Vec<Sub<T>>,
}

/// Holds the latest value and tells every subscriber about each change.
pub struct Hub<T: Clone> {
    inner: Mutex<Inner<T>>,
}

impl<T: Clone> Hub<T> {
    /// A hub whose first snapshot is `first`.
    pub fn new(first: T) -> Arc<Hub<T>> {
        Arc::new(Hub { inner: Mutex::new(Inner { latest: first, subs: Vec::new() }) })
    }

    /// Makes `value` the latest and sends it to every subscriber; one that cannot keep up is marked and will be given a fresh snapshot.
    pub fn publish(&self, value: T) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.latest = value.clone();
        inner.subs.retain(|s| match s.tx.try_send(value.clone()) {
            Err(TrySendError::Full(_)) => {
                s.gap.store(true, Ordering::Release);
                true
            }
            Err(TrySendError::Disconnected(_)) => false,
            Ok(()) => true,
        });
    }

    /// The latest value.
    pub fn latest(&self) -> T {
        self.inner.lock().unwrap_or_else(|e| e.into_inner()).latest.clone()
    }

    /// A subscription whose first update is the latest value.
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
#[derive(Debug, PartialEq)]
pub enum Update<T> {
    Next(T),
    Resnapshot(T),
}

/// An ordered stream of a hub's values: a snapshot first, then each change.
pub struct Subscription<T: Clone> {
    rx: Receiver<T>,
    gap: Arc<AtomicBool>,
    hub: Arc<Hub<T>>,
}

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

    /// The next update, waiting up to `wait`.
    pub fn next_within(&self, wait: Duration) -> Option<Update<T>> {
        self.try_next().or_else(|| self.rx.recv_timeout(wait).ok().map(Update::Next))
    }
}

/// Where a recording is in its life.
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Idle,
    Starting,
    Armed,
    Finishing,
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

/// Everything the window shows of one recording; `seq` grows with every change, `lease` names the recording.
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
    pub fn try_get(&self) -> Option<T> {
        self.0.try_recv().ok()
    }

    /// The answer, waiting up to `wait`.
    pub fn wait(&self, wait: Duration) -> Option<T> {
        self.0.recv_timeout(wait).ok()
    }
}
