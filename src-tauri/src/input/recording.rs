//! File: input/recording.rs
//! Purpose: One shortcut recording as a lease on the keyboard: Idle -> Starting (keys pass) -> Armed (every key is captured) -> Idle, with exactly one final event. Deadlines, the per-owner watermark, the outside-click regions and the hand-off of a finished combination to the ordered writer live here.
//! Contents: Lease — the running recording; the Core methods begin_lease, arm, end_lease, feed_recorder, regions, tick and next_deadline.

use std::sync::Arc;
use std::time::{Duration, Instant};

use super::core::Core;
use super::effects::Effect;
use super::gesture::{Ev, Gesture, Step};
use super::ledger::{Owner, Slot};
use super::popup::Rect;
use super::save::Job;
use super::view::{Event, Hub, Phase, Reason, RecorderView};

const START: Duration = Duration::from_secs(2);
const IDLE: Duration = Duration::from_secs(60);

/// The running recording.
pub struct Lease {
    pub epoch: u64,
    pub seq: u64,
    pub owner: u64,
    pub idx: usize,
    pub gesture: Gesture,
    pub hub: Arc<Hub<RecorderView>>,
    pub view: RecorderView,
    pub deadline: Instant,
    pub regions: Vec<Rect>,
}

impl Core {
    /// The epoch of the armed recording, if one is armed.
    pub(super) fn recorder_epoch(&self) -> Option<u64> {
        self.lease.as_ref().filter(|l| l.view.phase == Phase::Armed).map(|l| l.epoch)
    }

    /// Takes the pending request to arm a recording, for the driver to answer with `arm` or `fail_start`.
    pub fn arm_request(&mut self) -> Option<u64> {
        self.arm_request.take()
    }

    /// Publishes the lease's view again after a change made elsewhere.
    pub(super) fn republish_lease(&mut self) {
        self.publish_lease();
    }

    /// Publishes the lease's view with the next sequence number.
    fn publish_lease(&mut self) {
        let held = self.ledger.any_down();
        let saves = self.lease.as_ref().map_or(0, |l| self.pending.count_for(l.epoch));
        let Some(lease) = self.lease.as_mut() else { return };
        lease.view.seq += 1;
        lease.view.held = held;
        lease.view.saves = saves;
        let view = lease.view.clone();
        self.out.push(Effect::Recorder(lease.hub.clone(), view));
    }

    /// Starts a recording of shortcut `idx` for the owner's request `seq`. A request at or below the owner's watermark was cancelled first and is refused; a recording already running ends as replaced. Actions are released before it is acknowledged.
    pub(super) fn begin_lease(&mut self, owner: u64, seq: u64, idx: usize, hub: Arc<Hub<RecorderView>>, now: Instant) {
        let mut view = RecorderView {
            lease: seq,
            seq: 0,
            phase: Phase::Starting,
            open: true,
            parts: Vec::new(),
            live: false,
            held: false,
            error: None,
            saves: 0,
            revision: self.revision,
            event: Event::None,
        };
        let newer_open = self.lease.as_ref().is_some_and(|l| l.owner == owner && l.seq > seq);
        if newer_open || self.revoked.get(&owner).is_some_and(|&mark| seq <= mark) {
            view.phase = Phase::Idle;
            view.open = false;
            view.event = Event::Cancelled { reason: Reason::Revoked };
            self.out.push(Effect::Recorder(hub, view));
            return;
        }
        self.end_lease(Event::Cancelled { reason: Reason::Replaced }, false, now);
        self.retire_all();
        self.epochs += 1;
        let epoch = self.epochs;
        self.lease =
            Some(Lease { epoch, seq, owner, idx, gesture: Gesture::default(), hub, view, deadline: now + START, regions: Vec::new() });
        self.arm_request = Some(epoch);
        self.publish_lease();
    }

    /// The hook is confirmed: the recording starts capturing.
    pub fn arm(&mut self, epoch: u64, now: Instant) {
        let Some(lease) = self.lease.as_mut().filter(|l| l.epoch == epoch && l.view.phase == Phase::Starting) else { return };
        lease.view.phase = Phase::Armed;
        lease.deadline = now + IDLE;
        self.publish_lease();
    }

    /// The hook could not be made ready: the recording ends as failed.
    pub fn fail_start(&mut self, epoch: u64, message: String, now: Instant) {
        if self.lease.as_ref().is_some_and(|l| l.epoch == epoch) {
            self.end_lease(Event::Failed { message }, true, now);
        }
    }

    /// Ends the recording with its one final event. Presses it captured keep being swallowed until released, unless `override_drain`, where a stuck capture is worse than a stray key and everything passes.
    pub(super) fn end_lease(&mut self, event: Event, override_drain: bool, _now: Instant) {
        let Some(mut lease) = self.lease.take() else { return };
        lease.view.phase = Phase::Idle;
        lease.view.open = false;
        lease.view.event = event;
        self.lease = Some(lease);
        self.publish_lease();
        if let Some(lease) = self.lease.take() {
            self.ledger.reassign(Owner::Recorder(lease.epoch), Owner::Drain);
        }
        if override_drain {
            self.ledger.uncapture_all();
        }
    }

    /// Records that owner `owner` revoked every request up to `seq`, and ends its recording if that is one of them.
    pub(super) fn cancel_request(&mut self, owner: u64, seq: u64, reason: Reason, now: Instant) {
        let mark = self.revoked.entry(owner).or_insert(0);
        *mark = (*mark).max(seq);
        let current = self.lease.as_ref().is_some_and(|l| l.owner == owner && l.seq <= seq);
        if current {
            self.end_lease(Event::Cancelled { reason }, false, now);
        }
    }

    /// Hands the outside-click regions of recording `seq` of the owner to it; regions of an older recording are ignored.
    pub(super) fn set_regions(&mut self, owner: u64, seq: u64, rects: Vec<Rect>) {
        if let Some(l) = self.lease.as_mut().filter(|l| l.owner == owner && l.seq == seq) {
            l.regions = rects;
        }
    }

    /// Feeds an edge to the recording `epoch`. A press owned by an older recording only drains. Repeats never refresh the inactivity deadline.
    pub(super) fn feed_recorder(&mut self, epoch: u64, e: super::core::Edge, repeat: bool) {
        let held = self.ledger.held_mods();
        let Some(lease) = self.lease.as_mut().filter(|l| l.epoch == epoch) else { return };
        let ev = Ev { key: if let Slot::Key(k) = e.slot { Some(k) } else { None }, down: e.down, held };
        let step = lease.gesture.feed(ev);
        if !repeat && e.down {
            lease.deadline = self.now + IDLE;
        }
        lease.view.event = Event::None;
        match step {
            Step::Ignore => {}
            Step::Live { parts, keyed } => {
                lease.view.parts = parts;
                lease.view.live = keyed;
                lease.view.error = None;
            }
            Step::Clear => {
                lease.view.parts.clear();
                lease.view.live = false;
            }
            Step::Cancel => return self.end_lease(Event::Cancelled { reason: Reason::User }, false, self.now),
            Step::Commit(chord) => return self.submit_chord(epoch, chord),
        }
        self.publish_lease();
    }

    /// A finished combination: checked against the pending-latest set and queued for the writer, or refused with a message. The recording stays open.
    fn submit_chord(&mut self, epoch: u64, chord: super::chord::Chord) {
        let Some(idx) = self.lease.as_ref().map(|l| l.idx) else { return };
        self.tickets += 1;
        let job = Job { ticket: self.tickets, idx, chord, lease: Some(epoch), reply: None };
        let result = self.admit_write(job);
        if let Some(lease) = self.lease.as_mut() {
            lease.gesture.reset();
            if let Err(e) = result {
                lease.view.event = Event::Invalid { message: e.to_string() };
                lease.view.error = Some(e.to_string());
            }
        }
        self.publish_lease();
    }

    /// The nearest moment the input thread must wake for: the start or inactivity deadline of a recording, or the stall deadline of a popup. None when nothing is open, so an idle process has no timer.
    pub fn next_deadline(&self) -> Option<Instant> {
        [self.lease.as_ref().map(|l| l.deadline), self.popup.as_ref().map(|p| p.deadline)].into_iter().flatten().min()
    }

    /// Acts on deadlines that have passed.
    pub fn tick(&mut self, now: Instant) {
        self.now = now;
        self.expire_popup(now);
        let Some(lease) = self.lease.as_ref().filter(|l| now >= l.deadline) else { return };
        if lease.view.phase == Phase::Starting {
            let message = "Cue could not start listening for keys. Close this and try again.".to_string();
            return self.end_lease(Event::Failed { message }, true, now);
        }
        self.end_lease(Event::Cancelled { reason: Reason::Expired }, true, now);
    }
}
