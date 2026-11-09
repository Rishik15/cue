//! File: speech/events.rs
//! Purpose: What goes into and comes out of the dictation session machine, and the sentences it speaks to the user. Plain data: no process, no clock.
//! Contents: Pick — the model choice; Unload — the user's idle-unload setting; Event — what the shell feeds the session; Effect — what the session
//! asks the shell to do; the user-facing messages.

use std::time::Duration;

use super::protocol::{EngineSpec, Reply, Request};
use super::take::Done;

pub const STOPPED: &str = "The speech engine stopped before this recording finished.";
pub const NO_MODEL: &str = "No speech model installed. Open Settings, Models, and download one.";
pub const LOST: &str = "Cue lost track of this recording. Try dictating again.";
pub const CRASHED: &str = "The speech engine stopped unexpectedly. Try dictating again.";
pub const LOAD_SLOW: &str = "The speech model took too long to load.";
pub const RUN_SLOW: &str = "Transcription took too long and was stopped.";
pub const HUNG: &str = "The speech engine stopped responding.";
pub const OVERLOADED: &str = "The speech engine fell too far behind the recording.";
pub const MISMATCH: &str = "The speech engine is out of date. Restart Cue.";
pub const DROPPED: &str = "A recording ran too long without ending and was dropped.";

/// The model a worker loads: its catalog id and how the worker wires its files.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pick {
    pub id: &'static str,
    pub spec: &'static EngineSpec,
}

/// How long the engine stays loaded once nothing is in flight, as the user set it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Unload {
    /// Stay loaded until Cue quits.
    Never,
    /// Unload as soon as the last dictation is delivered.
    Immediately,
    /// Unload after this many seconds.
    After(u64),
}

impl Unload {
    /// The setting `unload_after_secs` as stored: negative never, zero immediately, otherwise seconds.
    pub fn from_setting(secs: i64) -> Self {
        match secs {
            s if s < 0 => Unload::Never,
            0 => Unload::Immediately,
            s => Unload::After(s as u64),
        }
    }
}

/// Everything that can happen to the session; the shell turns messages and timers into these.
pub enum Event {
    /// A dictation starts. `model` is the user's choice now (or why there is none); `cap` the longest recording they allow.
    Begin { id: u64, model: Result<Pick, String>, cap: Duration },
    /// 16 kHz audio for dictation `id`, possibly after the next dictation began.
    Audio { id: u64, samples: Vec<f32> },
    /// The user stopped dictation `id`; `done` gets the whole text.
    End { id: u64, done: Done },
    /// Escape: drop dictation `id` if it is still being recorded.
    Cancel { id: u64 },
    /// Kill the worker: the caller is about to delete its files.
    Unload,
    /// The worker with this serial said something.
    Reply { serial: u64, reply: Reply },
    /// The worker with this serial died or closed its pipe.
    Gone { serial: u64 },
    /// The worker with this serial could not be started.
    SpawnFailed { serial: u64, message: String },
    /// The input queue of the worker with this serial is full of audio it has not read.
    Overload { serial: u64 },
    /// The time passed a deadline.
    Tick,
}

/// What the session asks the shell to do, in order.
pub enum Effect {
    /// Start a worker process with this serial and begin loading the model.
    Spawn { serial: u64, pick: Pick },
    /// Write a request and its audio to the worker with this serial.
    Send { serial: u64, request: Request, samples: Vec<f32> },
    /// End the worker with this serial and wait for it.
    Kill { serial: u64 },
    /// Run a dictation's callback with its text or the message to show.
    Deliver(Done, Result<String, String>),
    /// Tell the user about a problem that no dictation callback will carry.
    Report(String),
    /// Show a clause as it is recognised: dictation, clause number, text.
    Partial { id: u64, index: usize, text: String },
    /// Hand freed memory back to Windows.
    Trim,
    /// A line for cue.log: timings per model, never what was said.
    Log(String),
}
