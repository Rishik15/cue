//! File: input/driver.rs
//! Purpose: The glue between the reducer and a platform: a `Runtime` owns the `Core` and the sink the effects go to, turns a pump's edges, commands, session events and deadlines into calls on the core, and flushes the effects. The Windows adapter and the scripted fake are two users of this one type.
//! Contents: ArmOutcome — how a driver answers a request to arm a recording; Runtime — `edge`, `process`, `session`, `tick` and `complete_arm`.

use std::sync::Arc;
use std::time::Instant;

use super::commands::Command;
use super::core::{Core, Edge, Verdict};
use super::effects::Sink;
use super::mailbox::Mailbox;

/// How the driver answers "make the hook ready for a recording".
pub enum ArmOutcome {
    Ready,
    Failed(String),
    Later,
}

/// A core, its sink and the way to ready the hook.
pub struct Runtime<S: Sink> {
    pub core: Core,
    pub sink: S,
    mailbox: Arc<Mailbox<Command>>,
    arm: Box<dyn FnMut() -> ArmOutcome + Send>,
    deferred: Option<u64>,
}

impl<S: Sink> Runtime<S> {
    /// A runtime over `core` that sends effects to `sink` and calls `arm` for each recording that needs the hook ready.
    pub fn new(core: Core, sink: S, mailbox: Arc<Mailbox<Command>>, arm: impl FnMut() -> ArmOutcome + Send + 'static) -> Runtime<S> {
        Runtime { core, sink, mailbox, arm: Box::new(arm), deferred: None }
    }

    /// Sends every queued effect. When one cannot be taken, the keyboard is opened and the surfaces end, and the verdict becomes pass.
    fn flush(&mut self, now: Instant, verdict: Verdict) -> Verdict {
        let mut lost = false;
        for effect in std::mem::take(&mut self.core.out) {
            lost |= !self.sink.send(effect);
        }
        if !lost {
            return verdict;
        }
        self.core.overflow(now);
        for effect in std::mem::take(&mut self.core.out) {
            let _ = self.sink.send(effect);
        }
        Verdict::Pass
    }

    /// Whether a recording is waiting for the hook to be made ready.
    pub fn deferred(&self) -> bool {
        self.deferred.is_some()
    }

    /// A callback panicked: the keyboard is opened and every surface and action ends.
    pub fn fault(&mut self, now: Instant) {
        self.core.overflow(now);
        self.flush(now, Verdict::Pass);
    }

    /// One key edge from the hook.
    pub fn edge(&mut self, e: Edge, now: Instant) -> Verdict {
        let verdict = self.core.edge(e, now);
        self.flush(now, verdict)
    }

    /// A pointer button went down at (x, y).
    pub fn pointer_down(&mut self, x: i32, y: i32, now: Instant) {
        self.core.pointer_down(x, y, now);
        self.flush(now, Verdict::Pass);
    }

    /// The key state may be wrong: lock, resume, a new desktop.
    pub fn session(&mut self, now: Instant) {
        self.core.session(now);
        self.flush(now, Verdict::Pass);
    }

    /// Deadlines.
    pub fn tick(&mut self, now: Instant) {
        self.core.tick(now);
        self.flush(now, Verdict::Pass);
    }

    /// Handles every queued command, then answers each request to arm a recording.
    pub fn process(&mut self, now: Instant) {
        for command in self.mailbox.take() {
            self.core.command(command, now);
        }
        while let Some(epoch) = self.core.arm_request() {
            match (self.arm)() {
                ArmOutcome::Ready => self.core.arm(epoch, now),
                ArmOutcome::Failed(message) => self.core.fail_start(epoch, message, now),
                ArmOutcome::Later => self.deferred = Some(epoch),
            }
        }
        self.flush(now, Verdict::Pass);
    }

    /// Completes the arm that was answered `Later`, if one waits.
    pub fn complete_arm(&mut self, outcome: ArmOutcome, now: Instant) {
        let Some(epoch) = self.deferred.take() else { return };
        match outcome {
            ArmOutcome::Ready => self.core.arm(epoch, now),
            ArmOutcome::Failed(message) => self.core.fail_start(epoch, message, now),
            ArmOutcome::Later => self.deferred = Some(epoch),
        }
        self.flush(now, Verdict::Pass);
    }
}
