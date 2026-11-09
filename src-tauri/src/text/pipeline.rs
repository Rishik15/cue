//! File: text/pipeline.rs
//! Purpose: The edit of a whole text as one call: cut it into units, ask a backend for each, keep only answers the reply check trusts, and join the result. Knows nothing about processes or HTTP.
//! Contents: Outcome — Edited, NoUsableAnswer, Cancelled or Failed; Pipeline — `run` (the unit loop) and `warm` (load the model and its prompt prefix ahead of a request).
//! Each unit is sent without hidden characters (tag characters can smuggle instructions); a rejected answer leaves that unit as written, so one bad answer never costs the rest.

use super::backend::Backend;
use super::cancel::Cancel;
use super::chunk::{self, Seg};
use super::engine::Fail;
use super::reply::{self, Verdict};
use crate::actions::Task;
use crate::sanitize::strip_hidden;

/// How an edit ended.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Edited(String),
    NoUsableAnswer,
    Cancelled,
    Failed(String),
}

/// A backend and the cancel token of the edit that uses it.
pub struct Pipeline<'a> {
    pub backend: &'a dyn Backend,
    pub cancel: &'a Cancel,
}

impl Pipeline<'_> {
    /// Edits `text` under `task`, unit by unit. Returns Edited with the joined text (units whose answer was rejected stay as written), NoUsableAnswer when every unit's answer was rejected,
    /// Cancelled when the token was set, or Failed with the backend's message.
    pub fn run(&self, task: &Task, text: &str) -> Outcome {
        let segs = chunk::split(text, task.unit);
        let (mut answers, mut kept) = (Vec::new(), 0);
        for unit in segs.iter().filter_map(|seg| if let Seg::Unit(unit) = seg { Some(*unit) } else { None }) {
            if self.cancel.is_set() {
                return Outcome::Cancelled;
            }
            let clean = strip_hidden(unit);
            let raw = match self.backend.ask(task, &clean, reply::max_tokens(&clean), self.cancel) {
                Ok(raw) => raw,
                Err(Fail::Cancelled) => return Outcome::Cancelled,
                Err(Fail::Failed(why)) => return Outcome::Failed(why),
            };
            match reply::accept(&clean, &raw, task.max_change) {
                Verdict::Accept(better) => answers.push(better),
                Verdict::Reject(reason) => {
                    eprintln!("cue: text answer rejected ({})", reason.code());
                    kept += 1;
                    answers.push(unit.to_string());
                }
            }
        }
        if !answers.is_empty() && kept == answers.len() {
            return Outcome::NoUsableAnswer;
        }
        Outcome::Edited(chunk::join(&segs, &answers))
    }

    /// Sends the task's prefix (instruction and examples) with a one-token answer, so the real request finds both the model loaded and the prefix cached. Returns nothing; a failure is left for the real request to report.
    pub fn warm(&self, task: &Task) {
        let _ = self.backend.ask(task, ".", 1, self.cancel);
    }
}
