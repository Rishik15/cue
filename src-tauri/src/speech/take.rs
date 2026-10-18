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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognised_audio_is_dropped_and_text_kept_in_order() {
        let mut t = Take::new(1);
        t.push(&[0.0; 1000]);
        assert_eq!(t.commit("one".into(), 400), 0);
        assert_eq!(t.seconds(), 600.0 / RATE as f32);
        assert_eq!(t.commit("two".into(), 900), 1);
        assert_eq!(t.text(), "one two");
    }

    #[test]
    fn a_long_silence_does_not_pile_up_and_counting_stays_right() {
        let mut t = Take::new(1);
        let r = RATE as usize;
        t.push(&vec![0.0; 100 * r]);
        assert_eq!(t.seconds(), KEEP_SECS as f32);
        // The worker counts from the start of the recording: 90 s in is 50 s into what is kept (40 s were dropped).
        t.commit("late".into(), 90 * r);
        assert_eq!(t.seconds(), 10.0);
    }

    #[test]
    fn a_replay_after_a_crash_continues_the_numbering_and_the_sample_count() {
        let mut t = Take::new(1);
        t.push(&[0.0; 1000]);
        t.commit("one".into(), 400);
        assert_eq!(t.rewind().len(), 600); // the fresh worker gets only what is left
        t.push(&[0.0; 200]);
        assert_eq!(t.commit("two".into(), 300), 1); // its sample 300 is 400 + 300 of the recording
        assert_eq!(t.seconds(), 500.0 / RATE as f32);
        assert_eq!(t.commit("three".into(), 10_000), 2); // never drops more than it holds
        assert_eq!(t.seconds(), 0.0);
    }

    #[test]
    fn a_crash_replays_parked_recordings_without_ending_them() {
        let mut running = VecDeque::from([Job { take: Take::new(1), done: Box::new(|_| ()) }]);
        let mut parked = [Session { take: Take::new(2), failed: None }];
        let mut current = Session { take: Take::new(3), failed: None };
        let sent = replay_all(&mut running, &mut parked, Some(&mut current));
        let ends = sent.iter().filter(|(r, _)| matches!(r, Request::End { .. })).count();
        let begins = sent.iter().filter(|(r, _)| matches!(r, Request::Begin { .. })).count();
        assert_eq!((begins, ends), (3, 1));
    }

    #[test]
    fn trimming_waits_for_the_slack() {
        let mut t = Take::new(1);
        t.push(&vec![0.0; (KEEP_SECS + 2) * RATE as usize]);
        assert_eq!(t.seconds(), (KEEP_SECS + 2) as f32);
    }
}
