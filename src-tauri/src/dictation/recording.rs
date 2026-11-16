//! File: dictation/recording.rs
//! Purpose: Which phase a dictation is in (idle, recording, stopping) and the field the text is for, changed only together under one lock. A recording can begin only from idle, so a press that arrives while the last recording is still being ended (its audio stop not issued yet) is ignored instead of starting a recording that the late stop would then end; whichever of Escape, a release or the recording limit ends a recording first gets its field and the others find nothing.
//! Contents: Phase — the state, the speech id of the recording and the field; Stopping — what the one that ended a recording holds until it has issued the audio stop; RECORDING — the one dictation's state.

use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::sync::Mutex;

use crate::platform::Target;

const IDLE: u8 = 0;
const RECORDING_NOW: u8 = 1;
const STOPPING: u8 = 2;

/// The phase of a dictation and the field its text is for.
pub struct Phase {
    phase: AtomicU8,
    /// The speech id of the recording in progress, 0 until it is published after `begin`. Changed only under the field's lock, so an ending that names an id acts on that recording or on nothing.
    id: AtomicU64,
    target: Mutex<Option<Target>>,
}

/// Held by whoever ended a recording until it has issued the audio stop (and the rest of the ending): dropping it, however the ending went, returns the phase to idle so the next press can start.
#[must_use]
pub struct Stopping<'a>(&'a Phase);

/// The one dictation's state.
pub static RECORDING: Phase = Phase::new();

impl Phase {
    /// An idle phase with no field.
    pub const fn new() -> Phase {
        Phase { phase: AtomicU8::new(IDLE), id: AtomicU64::new(0), target: Mutex::new(None) }
    }

    /// Whether a recording is running now (not while one is being ended).
    pub fn is_recording(&self) -> bool {
        self.phase.load(Ordering::Acquire) == RECORDING_NOW
    }

    /// Begins a recording from idle: the phase moves and the field is taken in one step. Input: how to take the field. Return: whether it began (false while a recording runs or is being ended).
    pub fn begin(&self, field: impl FnOnce() -> Target) -> bool {
        let mut slot = self.target.lock().unwrap_or_else(|e| e.into_inner());
        if self.phase.load(Ordering::Acquire) != IDLE {
            return false;
        }
        *slot = Some(field());
        self.id.store(0, Ordering::Release);
        self.phase.store(RECORDING_NOW, Ordering::Release);
        true
    }

    /// The speech id of the recording in progress (or the last one), 0 when it is not published yet.
    pub fn id(&self) -> u64 {
        self.id.load(Ordering::Acquire)
    }

    /// Names the running recording by its speech id; does nothing when it already ended. Input: the id.
    pub fn publish(&self, id: u64) {
        let _slot = self.target.lock().unwrap_or_else(|e| e.into_inner());
        if self.phase.load(Ordering::Acquire) == RECORDING_NOW {
            self.id.store(id, Ordering::Release);
        }
    }

    /// Ends the running recording: the phase moves to stopping and the field is handed over in one step. Return: the field and the hold on the stopping phase, or None when nothing is recording (another ending came first).
    pub fn end(&self) -> Option<(Target, Stopping<'_>)> {
        self.end_when(|_| true)
    }

    /// Ends the running recording only if it is the one with speech id `id`, checked and ended in one step: a late callback of an earlier recording finds nothing, even while the next one has begun and not published its id yet. Input: the id. Return: as `end`.
    pub fn end_for(&self, id: u64) -> Option<(Target, Stopping<'_>)> {
        self.end_when(|running| running == id)
    }

    /// Ends the running recording when `wanted` accepts its id, under the field's lock. Input: the test of the recording's id. Return: as `end`.
    fn end_when(&self, wanted: impl FnOnce(u64) -> bool) -> Option<(Target, Stopping<'_>)> {
        let mut slot = self.target.lock().unwrap_or_else(|e| e.into_inner());
        if self.phase.load(Ordering::Acquire) != RECORDING_NOW || !wanted(self.id.load(Ordering::Acquire)) {
            return None;
        }
        self.phase.store(STOPPING, Ordering::Release);
        Some((slot.take().unwrap_or_default(), Stopping(self)))
    }
}

impl Drop for Stopping<'_> {
    /// Returns the phase to idle.
    fn drop(&mut self) {
        let _slot = self.0.target.lock().unwrap_or_else(|e| e.into_inner());
        self.0.phase.store(IDLE, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::Phase;
    use crate::platform::Target;

    /// A field with a window number, to tell two recordings apart.
    fn field(window: isize) -> Target {
        Target { window, ..Target::default() }
    }

    /// The steps of the stop of recording 1 and of the press that starts recording 2, in every order they can interleave (each actor's own steps stay in order). The stop: end, issue the audio stop, release the hold. The press: begin, issue the audio start. Whatever the order, the audio stop of recording 1 is issued before the audio start of recording 2, recording 2 never gets recording 1's field, and the stop always ends the recording it was for.
    #[test]
    fn a_stop_and_a_new_press_in_every_order_never_cross() {
        let orders = interleavings(3, 2);
        assert_eq!(orders.len(), 10);
        for order in orders {
            let phase = Phase::new();
            assert!(phase.begin(|| field(1)));
            let (mut stop, mut press) = (0, 0);
            let (mut hold, mut got, mut log, mut began) = (None, None, Vec::new(), false);
            for actor in order {
                if actor == 0 {
                    match stop {
                        0 => {
                            let ended = phase.end().expect("recording 1 is running");
                            got = Some(ended.0.window);
                            hold = Some(ended.1);
                        }
                        1 => log.push("audio stop 1"),
                        _ => drop(hold.take()),
                    }
                    stop += 1;
                } else {
                    match press {
                        0 => began = phase.begin(|| field(2)),
                        _ if began => log.push("audio start 2"),
                        _ => {}
                    }
                    press += 1;
                }
            }
            assert_eq!(got, Some(1));
            if began {
                assert_eq!(log, ["audio stop 1", "audio start 2"], "order {log:?}");
                assert_eq!(phase.end().map(|e| e.0.window), Some(2));
            } else {
                assert_eq!(log, ["audio stop 1"]);
            }
        }
    }

    /// Every way to merge two step lists of lengths `a` and `b`, each as the list of which actor (0 or 1) takes the next step.
    fn interleavings(a: usize, b: usize) -> Vec<Vec<u8>> {
        if a == 0 {
            return vec![vec![1; b]];
        }
        if b == 0 {
            return vec![vec![0; a]];
        }
        let mut all = Vec::new();
        for mut rest in interleavings(a - 1, b) {
            rest.insert(0, 0);
            all.push(rest);
        }
        for mut rest in interleavings(a, b - 1) {
            rest.insert(0, 1);
            all.push(rest);
        }
        all
    }

    /// A late ending that names an earlier recording's id never ends a newer recording, also while the newer one has begun but not published its id; the right id ends it.
    #[test]
    fn an_ending_for_an_earlier_recording_leaves_the_newer_one_running() {
        let phase = Phase::new();
        assert!(phase.begin(|| field(1)));
        phase.publish(10);
        drop(phase.end_for(10).expect("its own ending"));
        assert!(phase.begin(|| field(2)));
        assert!(phase.end_for(10).is_none(), "newer recording has no id yet");
        phase.publish(11);
        assert!(phase.end_for(10).is_none() && phase.is_recording());
        assert_eq!(phase.end_for(11).map(|e| e.0.window), Some(2));
    }

    /// Escape and a release ending the same recording at once: exactly one of them gets the field, whichever is first, and the phase is idle again once its hold is dropped.
    #[test]
    fn two_endings_of_one_recording_give_the_field_to_one() {
        let phase = Phase::new();
        assert!(phase.begin(|| field(7)));
        let first = phase.end().expect("the first ending");
        assert!(phase.end().is_none(), "the second finds nothing");
        assert!(!phase.is_recording() && !phase.begin(|| field(8)), "no new recording while one is being ended");
        drop(first);
        assert!(phase.begin(|| field(8)));
    }

    /// Threads hammering begin and end never hand out one field twice, never end a recording that was not running and always return to idle.
    #[test]
    fn begin_and_end_from_many_threads_stay_consistent() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let phase = Phase::new();
        let (begun, ended) = (AtomicUsize::new(0), AtomicUsize::new(0));
        std::thread::scope(|s| {
            for t in 0..4 {
                let (phase, begun, ended) = (&phase, &begun, &ended);
                s.spawn(move || {
                    for i in 0..5_000 {
                        if t % 2 == 0 {
                            if phase.begin(|| field(i)) {
                                begun.fetch_add(1, Ordering::SeqCst);
                            }
                        } else if let Some((_, hold)) = phase.end() {
                            ended.fetch_add(1, Ordering::SeqCst);
                            drop(hold);
                        }
                    }
                });
            }
        });
        let (b, e) = (begun.load(Ordering::SeqCst), ended.load(Ordering::SeqCst));
        assert!(e <= b && b - e <= 1, "begun {b}, ended {e}");
        assert_eq!(phase.is_recording(), b > e);
    }
}
