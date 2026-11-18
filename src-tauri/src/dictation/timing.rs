//! File: dictation/timing.rs
//! Purpose: Where the wait between letting go of the key and the text went, as one log line per dictation: stage names and milliseconds only, never what was said. Marks are taken only while a dictation runs; nothing runs at idle.
//! Contents: Timeline — the release time and the marks taken since; line — the log line.

use std::time::Instant;

/// The stages of one stop, each marked with the milliseconds since the key was released.
#[derive(Clone)]
pub struct Timeline {
    start: Instant,
    marks: Vec<(&'static str, u64)>,
}

impl Timeline {
    /// Starts the clock at the release (or at the press that stops a Toggle recording).
    pub fn start() -> Timeline {
        Timeline { start: Instant::now(), marks: Vec::new() }
    }

    /// Records that `stage` was reached now. Input: the stage name.
    pub fn mark(&mut self, stage: &'static str) {
        self.marks.push((stage, self.start.elapsed().as_millis() as u64));
    }

    /// The log line for this dictation. Inputs: how long was spoken in ms and how many threads the engine used.
    pub fn line(&self, spoken_ms: u64, threads: i32) -> String {
        line(spoken_ms, threads, &self.marks)
    }
}

/// Formats the stage marks. Inputs: ms spoken, engine threads and the (stage, ms since release) marks in order. Returns the line for `cue.log`.
pub fn line(spoken_ms: u64, threads: i32, marks: &[(&str, u64)]) -> String {
    let stages: Vec<String> = marks.iter().map(|(stage, ms)| format!("{stage} {ms}")).collect();
    format!("dictation: {spoken_ms} ms spoken, {threads} threads, ms after release: {}", stages.join(", "))
}
