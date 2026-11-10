//! File: speech/take.rs
//! Purpose: One dictation as the app keeps it: the clauses already recognised plus only the audio the worker has not released yet, so memory
//! stays flat however long the recording runs and a crash can replay just that rest without repeating or losing a clause.
//! Contents: Take — id, committed clauses, unreleased audio; push / commit / release / replay / text / seconds; Done — what runs with the text.

use super::protocol::Request;
use crate::audio::RATE;

/// More than the worker keeps (40 s), so a replay never lacks audio the worker still had.
const KEEP_SECS: usize = 60;
/// Trimming moves the whole buffer, so it waits until this much is over the limit instead of shifting it on every chunk.
const SLACK_SECS: usize = 5;

/// Runs when a dictation's text is ready (or it failed with nothing recognised).
pub type Done = Box<dyn FnOnce(Result<String, String>) + Send>;

/// One dictation as the app keeps it: its committed clauses and the audio the worker has not released.
pub struct Take {
    pub id: u64,
    audio: Vec<f32>,
    /// Where `audio` starts in the whole recording, and where the worker's own count of samples starts (it restarts after a replay).
    base: usize,
    origin: usize,
    committed: Vec<String>,
    /// How many clauses were committed when the audio was last released: a replay starts there, so it numbers its clauses from it.
    released_clauses: usize,
}

impl Take {
    /// A dictation with nothing heard yet. Input: its id.
    pub fn new(id: u64) -> Self {
        Take { id, audio: Vec::new(), base: 0, origin: 0, committed: Vec::new(), released_clauses: 0 }
    }

    /// Keeps the audio the worker has not released. Only the latest `KEEP_SECS` are kept: the worker drops audio older than that too, so a
    /// recording with no release for a minute (a streaming model that hears no pause) replays only its last minute after a crash.
    /// shortcut: the oldest audio past the cap is dropped, raise the cap or release at fixed points if a longer exact replay is ever needed.
    pub fn push(&mut self, samples: &[f32]) {
        self.audio.extend_from_slice(samples);
        let excess = self.audio.len().saturating_sub(RATE as usize * KEEP_SECS);
        if excess > RATE as usize * SLACK_SECS {
            self.audio.drain(..excess);
            self.base += excess;
        }
    }

    /// A clause came back from the worker. Inputs: its number and text. Returns its number when it is new; a clause already committed (a
    /// replay recognises the same words again) is ignored, so the first text of a clause stands.
    pub fn commit(&mut self, index: usize, text: String) -> Option<usize> {
        if index < self.committed.len() {
            return None;
        }
        self.committed.push(text);
        Some(self.committed.len() - 1)
    }

    /// The worker will not need the audio before its sample `up_to`: stop keeping it, and remember how many clauses it produced (`clauses`, as
    /// the worker counted them). Clauses committed after the release point come from audio still kept, so they are not counted: the replay will
    /// recognise that audio again and number its clauses from the released count. The count only grows and never passes what is committed.
    pub fn release(&mut self, up_to: usize, clauses: usize) {
        let used = self.origin.saturating_add(up_to).saturating_sub(self.base).min(self.audio.len());
        self.audio.drain(..used);
        self.base += used;
        self.released_clauses = self.released_clauses.max(clauses.min(self.committed.len()));
    }

    /// What a fresh worker is sent so it continues this dictation: its Begin numbered after the released clauses, the unreleased audio, and
    /// the End when the user has already stopped. The worker's sample count starts again at the first replayed sample.
    pub fn replay(&mut self, finished: bool) -> Vec<(Request, Vec<f32>)> {
        let id = self.id;
        self.origin = self.base;
        let mut requests = vec![(Request::Begin { id, clause: self.released_clauses }, Vec::new())];
        if !self.audio.is_empty() {
            requests.push((Request::Audio, self.audio.clone()));
        }
        if finished {
            requests.push((Request::End { id }, Vec::new()));
        }
        requests
    }

    /// The whole text so far, clauses joined by a space.
    pub fn text(&self) -> String {
        self.committed.join(" ")
    }

    /// Length in seconds of the audio still unreleased.
    pub fn seconds(&self) -> f32 {
        self.audio.len() as f32 / RATE as f32
    }
}
