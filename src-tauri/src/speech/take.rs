//! Purpose: One dictation as the supervisor sees it: the sentences already recognised plus only the audio the worker has not used up yet,
//! so memory stays flat however long the recording runs and a crash can replay just that rest without repeating or losing a sentence.
//! Contents: Take — id, committed sentences, unrecognised audio; push / commit / replay / text / seconds.

use super::protocol::Request;
use crate::audio::RATE;

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

    pub fn push(&mut self, samples: &[f32]) {
        self.audio.extend_from_slice(samples);
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
}
