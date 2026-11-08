//! File: input/commands.rs
//! Purpose: Requests from other threads, handled in order on the input thread: start and cancel a recording, reset a shortcut, finish a save, re-read the on/off switches, open and close the tray popup. Saving lives here too: a combination is admitted to the ordered writer and activates only after the write is durable.
//! Contents: Command — every request; the Core methods `command`, `admit_write` and `settle`.

use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::Instant;

use super::chord::Chord;
use super::core::Core;
use super::effects::Effect;
use super::error::InputError;
use super::popup::{Popup, Rect};
use super::save::{Done, Job, Settled};
use super::view::{Event, Hub, Reason, RecorderView};

/// One request to the input thread.
pub enum Command {
    Record { owner: u64, seq: u64, idx: usize, hub: Arc<Hub<RecorderView>> },
    Cancel { owner: u64, seq: u64, reason: Reason, reply: Option<Sender<()>> },
    Regions { owner: u64, rects: Vec<Rect> },
    OwnerGone { owner: u64 },
    Reset { idx: usize, reply: Sender<Result<u64, InputError>> },
    SaveDone { ticket: u64, result: Result<(), String> },
    RefreshEnabled,
    Recover { reply: Sender<Result<(), InputError>> },
    OpenPopup(Popup),
    ClosePopup { id: u64 },
}

impl Core {
    /// Handles one request.
    pub fn command(&mut self, command: Command, now: Instant) {
        self.now = now;
        match command {
            Command::Record { owner, seq, idx, hub } => self.begin_lease(owner, seq, idx, hub, now),
            Command::Cancel { owner, seq, reason, reply } => {
                self.cancel_request(owner, seq, reason, now);
                if let Some(reply) = reply {
                    let _ = reply.send(());
                }
            }
            Command::Regions { owner, rects } => self.set_regions(owner, rects),
            Command::OwnerGone { owner } => {
                self.cancel_request(owner, u64::MAX, Reason::WindowClosed, now);
                self.revoked.remove(&owner);
            }
            Command::Reset { idx, reply } => self.reset(idx, reply, now),
            Command::SaveDone { ticket, result } => self.settle(ticket, result),
            Command::RefreshEnabled => self.refresh_enabled(),
            Command::Recover { reply } => {
                self.session(now);
                if let Some(old) = self.recover_reply.replace(reply) {
                    let _ = old.send(Err(InputError::Busy));
                }
            }
            Command::OpenPopup(popup) => self.open_popup(popup, now),
            Command::ClosePopup { id } => self.close_popup(id),
        }
    }

    /// Puts a shortcut back to its default. A recording of it ends. The answer is the new revision, sent once the write is durable.
    fn reset(&mut self, idx: usize, reply: Sender<Result<u64, InputError>>, now: Instant) {
        let (mods, key) = self.catalog.specs[idx].default;
        let Some(chord) = Chord::new(mods, Some(key)) else { return };
        if self.lease.as_ref().is_some_and(|l| l.idx == idx) {
            self.end_lease(Event::Cancelled { reason: Reason::Reset }, false, now);
        }
        self.tickets += 1;
        let job = Job { ticket: self.tickets, idx, chord, lease: None, reply: Some(reply) };
        let _ = self.admit_write(job);
    }

    /// Queues a write after checking it against the pending-latest set. The refusal is returned and, for a reset, also answered.
    pub(super) fn admit_write(&mut self, job: Job) -> Result<(), InputError> {
        let specs = self.catalog.specs;
        let (idx, chord, ticket) = (job.idx, job.chord, job.ticket);
        if let Err((error, job)) = self.pending.admit(specs, job) {
            if let Some(reply) = &job.reply {
                let _ = reply.send(Err(error.clone()));
            }
            return Err(error);
        }
        let _ = (idx, chord, ticket);
        self.writes_due = true;
        Ok(())
    }

    /// Whether a queued write waits to be handed to the writer. The input thread's message loop checks this after each callback and calls `drive_writes`, so no thread is started inside a hook.
    pub fn writes_due(&self) -> bool {
        self.writes_due && self.in_flight.is_none() && self.pending.head().is_some()
    }

    /// Hands the oldest queued write to the writer when none is being written. If the writer cannot take it, the write fails like a failed save.
    pub fn drive_writes(&mut self) {
        self.writes_due = false;
        if self.in_flight.is_some() {
            return;
        }
        let specs = self.catalog.specs;
        let Some((ticket, idx, chord)) = self.pending.head().map(|j| (j.ticket, j.idx, j.chord)) else { return };
        let mailbox = self.mailbox.clone();
        let done: Done = Arc::new(move |ticket, result| mailbox.post(Command::SaveDone { ticket, result }));
        match self.writer.submit(&self.store, done, (ticket, specs[idx].id, chord)) {
            Ok(()) => self.in_flight = Some(ticket),
            Err(error) => {
                let rolled = self.pending.finish(specs, ticket, Err(error.to_string()));
                rolled.into_iter().for_each(|s| self.settled(s));
                self.publish_catalog();
                self.writes_due = true;
            }
        }
    }

    /// A write finished: commit or drop it and everything the outcome decides.
    fn settle(&mut self, ticket: u64, result: Result<(), String>) {
        if self.in_flight == Some(ticket) {
            self.in_flight = None;
        }
        self.writes_due = true;
        let settled = self.pending.finish(self.catalog.specs, ticket, result);
        settled.into_iter().for_each(|s| self.settled(s));
        self.publish_catalog();
    }

    /// Applies one finished job: a durable write activates the binding and bumps the revision; a failed one is reported.
    fn settled(&mut self, s: Settled) {
        let Settled { job, result } = s;
        let idx = job.idx;
        match &result {
            Ok(()) => {
                self.bindings[idx].chord = job.chord;
                self.bindings[idx].valid = true;
                self.errors[idx] = None;
                self.revision += 1;
                self.last_error = None;
                self.retire_action(idx);
            }
            Err(e) => {
                self.last_error = Some(e.to_string());
                let shown = job.reply.is_some() || self.lease.as_ref().is_some_and(|l| Some(l.epoch) == job.lease);
                if !shown {
                    self.out.push(Effect::Report(format!("A shortcut could not be saved: {e}")));
                }
            }
        }
        if let (Some(epoch), Some(lease)) = (job.lease, self.lease.as_mut()) {
            if lease.epoch == epoch {
                lease.view.revision = self.revision;
                lease.view.event = match &result {
                    Ok(()) => Event::Saved,
                    Err(e) => Event::Invalid { message: e.to_string() },
                };
            }
        }
        if let Some(reply) = job.reply {
            let _ = reply.send(result.map(|()| self.revision));
        }
        if self.lease.as_ref().is_some_and(|l| Some(l.epoch) == job.lease) {
            self.republish_lease();
        }
    }

    /// Re-reads the on/off switches; a shortcut that went off is released and its keys pass again.
    fn refresh_enabled(&mut self) {
        for idx in 0..self.bindings.len() {
            let on = self.store.enabled(self.catalog.specs[idx].id);
            if self.bindings[idx].enabled != on {
                self.bindings[idx].enabled = on;
                self.retire_action(idx);
            }
        }
        self.publish_catalog();
    }
}
