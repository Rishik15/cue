//! File: input/lift.rs
//! Purpose: Cue's own keystrokes while the user still holds a shortcut: the held modifiers are lifted, Cue's keys go out, and the modifiers are put back only if they are still physically held in the same epoch. One fallible transaction on the caller's thread, so it may span paced typing and paste waits; the hook keeps running on the input thread and ignores the marked keys.
//! Contents: KeyStep / KeyBatch / SendReceipt / RestoreReport — the keys and what happened; plan — the lift and restore steps for a held set; LiftGuard — the open transaction; lift — open one (one at a time in the process).

use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use super::error::InputError;
use super::key::Mods;
use super::shared::{global, Shared};

/// One key event to send: a virtual key (or a UTF-16 unit when `unicode`), down or up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyStep {
    pub vk: u16,
    pub scan: u16,
    pub up: bool,
    pub extended: bool,
    pub unicode: bool,
}

/// Key events sent in one call.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KeyBatch(pub Vec<KeyStep>);

/// How many events of a batch the system took.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SendReceipt {
    pub sent: usize,
    pub wanted: usize,
}

/// What putting the modifiers back did: the ones restored, and the ones released again because the user let go meanwhile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RestoreReport {
    pub restored: Mods,
    pub released: Mods,
}

const MENU_MASK: u16 = 0xE8;

const KEYS: [(Mods, u16, bool); 8] = [
    (Mods::WIN_L, 0x5B, true),
    (Mods::WIN_R, 0x5C, true),
    (Mods::SHIFT_L, 0xA0, false),
    (Mods::SHIFT_R, 0xA1, false),
    (Mods::CTRL_L, 0xA2, false),
    (Mods::CTRL_R, 0xA3, true),
    (Mods::ALT_L, 0xA4, false),
    (Mods::ALT_R, 0xA5, true),
];

/// A key step for a virtual key.
fn step(vk: u16, up: bool, extended: bool) -> KeyStep {
    KeyStep { vk, scan: 0, up, extended, unicode: false }
}

/// The steps that lift `held` and the ones that put it back. Alt and Win get a menu mask before they are lifted and after they are pressed again: the shell opens a menu when Alt or Win goes down and up with no other key between.
pub fn plan(held: Mods) -> (KeyBatch, KeyBatch) {
    let down: Vec<_> = KEYS.iter().filter(|(flag, ..)| held.has(*flag)).collect();
    let mask = held.any(Mods::ALT_L | Mods::ALT_R | Mods::WIN_L | Mods::WIN_R);
    let tap = || [step(MENU_MASK, false, false), step(MENU_MASK, true, false)];
    let mut lift: Vec<KeyStep> = if mask { tap().to_vec() } else { Vec::new() };
    lift.extend(down.iter().map(|(_, vk, ext)| step(*vk, true, *ext)));
    let mut restore: Vec<KeyStep> = down.iter().rev().map(|(_, vk, ext)| step(*vk, false, *ext)).collect();
    if mask {
        restore.extend(tap());
    }
    (KeyBatch(lift), KeyBatch(restore))
}

static LIFTING: Mutex<()> = Mutex::new(());

/// Sends a batch through the platform and checks every event went out.
fn send_all(batch: &KeyBatch) -> Result<SendReceipt, InputError> {
    let wanted = batch.0.len();
    let sent = if wanted == 0 { 0 } else { crate::platform::send_keys(batch) };
    if sent == wanted {
        Ok(SendReceipt { sent, wanted })
    } else {
        Err(InputError::InjectionPartial { sent, wanted })
    }
}

/// An open transaction: the modifiers held at the start stay lifted until `finish` or drop.
pub struct LiftGuard {
    lifted: Mods,
    epoch: u64,
    shared: &'static Shared,
    done: bool,
    _one_at_a_time: MutexGuard<'static, ()>,
}

impl LiftGuard {
    /// Puts back the modifiers still physically held in the same epoch, releases again any the user let go while they were being put back, and reports both.
    pub fn finish(mut self) -> Result<RestoreReport, InputError> {
        self.restore()
    }

    /// The restore, once. A changed epoch means releases may have been missed, so nothing is pressed again.
    fn restore(&mut self) -> Result<RestoreReport, InputError> {
        let none = RestoreReport { restored: Mods::NONE, released: Mods::NONE };
        if std::mem::replace(&mut self.done, true) || self.lifted.is_empty() || self.shared.epoch() != self.epoch {
            return Ok(none);
        }
        let shared = self.shared;
        let still = Mods(self.lifted.0 & shared.held().0);
        let (report, failure) = reconcile(still, &|| shared.held(), &mut |batch| match send_all(batch) {
            Ok(r) => (r.sent, None),
            Err(e) => (partial_sent(&e), Some(e)),
        });
        failure.map_or(Ok(report), Err)
    }
}

/// How many events a failed send took, from its error.
fn partial_sent(error: &InputError) -> usize {
    match error {
        InputError::InjectionPartial { sent, .. } => *sent,
        _ => 0,
    }
}

/// The modifiers whose down event is among the first `sent` events of `batch`.
fn pressed_in(batch: &KeyBatch, sent: usize) -> Mods {
    batch
        .0
        .iter()
        .take(sent)
        .filter(|s| !s.up)
        .filter_map(|s| KEYS.iter().find(|(_, vk, _)| *vk == s.vk && s.vk != MENU_MASK).map(|(m, ..)| *m))
        .fold(Mods::NONE, |all, m| all | m)
}

/// The modifiers whose up event is among the first `sent` events of `batch`.
fn released_in(batch: &KeyBatch, sent: usize) -> Mods {
    batch
        .0
        .iter()
        .take(sent)
        .filter(|s| s.up)
        .filter_map(|s| KEYS.iter().find(|(_, vk, _)| *vk == s.vk && s.vk != MENU_MASK).map(|(m, ..)| *m))
        .fold(Mods::NONE, |all, m| all | m)
}

/// Presses `still` again and then releases every one the user let go of while that happened, even when a send fails part way: what a failed send did take is released like the rest, so no modifier is left logically down after the physical key is up. A release that was not sent stays owed and is tried again on the next round. `send` returns how many events it sent and the error if it failed. The report says what was restored and released; the error, if any, is the first failure.
pub(super) fn reconcile(
    still: Mods,
    held: &dyn Fn() -> Mods,
    send: &mut dyn FnMut(&KeyBatch) -> (usize, Option<InputError>),
) -> (RestoreReport, Option<InputError>) {
    let restore = plan(still).1;
    let (sent, mut failure) = send(&restore);
    let mut pressed = pressed_in(&restore, sent);
    let mut released = Mods::NONE;
    for wait_ms in [4, 16] {
        std::thread::sleep(Duration::from_millis(wait_ms));
        let gone = Mods(pressed.0 & !held().0);
        if gone.is_empty() {
            continue;
        }
        let batch = plan(gone).0;
        let (sent, error) = send(&batch);
        failure = failure.or(error);
        let up = released_in(&batch, sent);
        pressed = pressed.without(up);
        released = released.with(up);
    }
    (RestoreReport { restored: pressed, released }, failure)
}

impl Drop for LiftGuard {
    /// Best-effort restore when the caller did not call `finish`.
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

/// Opens a lift: the modifiers Windows reports down are lifted unless the focused app is elevated (the events would be dropped and leave the keys stuck). One lift at a time; a second waits for the first. A lift that could only be partly sent is undone as far as possible before the error returns.
pub fn lift() -> Result<LiftGuard, InputError> {
    let shared = global().ok_or_else(|| InputError::Unavailable("Cue's keyboard is not running.".into()))?;
    let guard = LIFTING.lock().unwrap_or_else(|e| e.into_inner());
    let held = if crate::platform::lift_blocked() { Mods::NONE } else { crate::platform::os_modifiers() };
    let mut lift = LiftGuard { lifted: Mods::NONE, epoch: shared.epoch(), shared, done: false, _one_at_a_time: guard };
    if held.is_empty() {
        return Ok(lift);
    }
    lift.lifted = held;
    send_all(&plan(held).0)?;
    Ok(lift)
}
