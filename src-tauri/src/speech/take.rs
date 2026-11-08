//! Purpose: One dictation as the supervisor sees it: the sentences already recognised plus only the audio the worker has not used up yet,
//! so memory stays flat however long the recording runs and a crash can replay just that rest without repeating or losing a sentence.
//! Contents: Take — id, committed sentences, unrecognised audio; push / commit / replay / text / seconds; Session and Job — a take while it is
//! recorded and while its last words are recognised; Done — what runs with the text; replay_all — what a fresh worker is sent after a crash.

use std::collections::VecDeque;

use super::protocol::Request;
use crate::audio::RATE;

/// More than the worker keeps (40 s), so a replay never lacks audio the worker still had.
const KEEP_SECS: usize = 60;
/// Trimming moves the whole buffer, so it waits until this much is over the limit instead of shifting it on every chunk.
const SLACK_SECS: usize = 5;

/// Runs when a dictation's text is ready (or it failed with nothing recognised).
pub type Done = Box<dyn FnOnce(Result<String, String>) + Send>;

/// A finished recording waiting for the rest of its text. Its unrecognised audio is kept so a crash can replay it on a fresh worker.
pub struct Job {
    pub take: Take,
    pub done: Done,
}

/// The dictation being recorded right now.
pub struct Session {
    pub take: Take,
    /// The worker could not start; reported when the dictation ends.
    pub failed: Option<String>,
}

/// Everything a fresh worker needs after a crash: stopped recordings (ended), recordings stopped but not yet ended (parked, whose `end`
/// will still arrive) and the one being recorded.
pub fn replay_all(running: &mut VecDeque<Job>, parked: &mut [Session], session: Option<&mut Session>) -> Vec<(Request, Vec<f32>)> {
    let jobs = running.iter_mut().map(|j| (&mut j.take, true));
    let open = parked.iter_mut().chain(session).map(|s| (&mut s.take, false));
    jobs.chain(open).flat_map(|(take, finished)| take.replay(finished)).collect()
}

pub struct Take {
    pub id: u64,
    audio: Vec<f32>,
    /// Where `audio` starts in the whole recording, and where the worker's own count of samples starts (it restarts after a replay).
    base: usize,
    origin: usize,
    committed: Vec<String>,
}

impl Take {
    pub fn new(id: u64) -> Self {
        Take { id, audio: Vec::new(), base: 0, origin: 0, committed: Vec::new() }
    }

    /// Only the latest `KEEP_SECS` are kept: the worker drops audio older than that too (it was recognised, or it was silence), so a long
    /// silence in a lecture does not pile up here.
    pub fn push(&mut self, samples: &[f32]) {
        self.audio.extend_from_slice(samples);
        let excess = self.audio.len().saturating_sub(RATE as usize * KEEP_SECS);
        if excess > RATE as usize * SLACK_SECS {
            self.audio.drain(..excess);
            self.base += excess;
        }
    }

    /// A sentence came back from the worker, which has used up the audio to its sample `end`. Returns the sentence's number in the whole
    /// recording: a fresh worker after a crash counts from 0 again.
    pub fn commit(&mut self, text: String, end: usize) -> usize {
        let used = (self.origin + end).saturating_sub(self.base).min(self.audio.len());
        self.audio.drain(..used);
        self.base += used;
        self.committed.push(text);
        self.committed.len() - 1
    }

    /// What a fresh worker is sent after a crash: this dictation from the audio not yet recognised on (its sample count starts again there),
    /// ended too when the user has already stopped.
    pub fn replay(&mut self, finished: bool) -> Vec<(Request, Vec<f32>)> {
        let id = self.id;
        let mut requests = vec![(Request::Begin { id }, Vec::new()), (Request::Audio, self.rewind())];
        if finished {
            requests.push((Request::End { id }, Vec::new()));
        }
        requests
    }

    fn rewind(&mut self) -> Vec<f32> {
        self.origin = self.base;
        self.audio.clone()
    }

    pub fn text(&self) -> String {
        self.committed.join(" ")
    }

    /// Length of the audio still waiting to be recognised.
    pub fn seconds(&self) -> f32 {
        self.audio.len() as f32 / RATE as f32
    }
}
