//! File: text/pipeline.rs
//! Purpose: The edit of a whole text as one call: size each request to the model's context, cut oversized units into windows, ask a backend for each, keep only answers the checks trust, and join the result. Knows nothing about processes or HTTP.
//! Contents: Outcome — Edited (with an optional notice), NoUsableAnswer (with the reason), Cancelled or Failed; Pipeline — `run` (editing: the unit loop; whole-text tasks: the head of the text in one request) and `warm` (load the model and its prompt prefix ahead of a request).
//! Each unit is sent without hidden characters (tag characters can smuggle instructions); a rejected answer leaves that unit as written, so one bad answer never costs the rest. Nothing is refused for being long: editing stops after `total_chars` and says so, a whole-text task uses the head that fits and says so.

use super::backend::Backend;
use super::budget::{self, Plan};
use super::cancel::Cancel;
use super::chunk::{self, Seg};
use super::engine::Fail;
use super::mask;
use super::reply::{self, Reason, Verdict};
use super::window;
use crate::actions::{Scope, Task};
use crate::sanitize::strip_hidden;

/// How an edit ended.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Edited { text: String, notice: Option<String> },
    NoUsableAnswer(Reason),
    Cancelled,
    Failed(String),
}

/// What became of one unit: its answer was used, or the original stays and this is why.
enum Answer {
    Used(String),
    Kept(Reason),
}

/// A backend and the cancel token of the edit that uses it.
pub struct Pipeline<'a> {
    backend: &'a dyn Backend,
    cancel: &'a Cancel,
    total_chars: usize,
}

impl<'a> Pipeline<'a> {
    /// A pipeline over `backend` that stops editing after the default number of characters. Inputs: the backend and the cancel token. Returns it.
    pub fn new(backend: &'a dyn Backend, cancel: &'a Cancel) -> Self {
        Self { backend, cancel, total_chars: budget::EDIT_TOTAL_CHARS }
    }

    /// The same pipeline with another limit on the characters an editing run works through. Input: the limit. Returns the pipeline.
    #[cfg(test)]
    pub fn with_total_chars(mut self, total_chars: usize) -> Self {
        self.total_chars = total_chars;
        self
    }

    /// Edits `text` under `task`. Returns Edited with the text (units whose answer was rejected stay as written) and a notice when part of the text was not processed,
    /// NoUsableAnswer with the first reason when every answer was rejected, Cancelled when the token was set, or Failed with the backend's message.
    pub fn run(&self, task: &Task, text: &str) -> Outcome {
        let plan = Plan::new(self.backend.context(), self.backend.prefix_tokens(task), task.scope);
        match task.scope {
            Scope::Units => self.edit_units(task, text, &plan),
            Scope::Whole => self.answer_whole(task, text, &plan),
        }
    }

    /// Sends the task's prefix (instruction and examples) with a one-token answer, so the real request finds both the model loaded and the prefix cached. Returns nothing; a failure is left for the real request to report.
    /// It runs on the caller's thread with no thread per request: the one-token answer is over before a cancel could matter, and the cancel token is still honoured by the start.
    pub fn warm(&self, task: &Task) {
        let _ = self.backend.ask(task, ".", 1, self.cancel);
    }

    /// The editing loop: the units in order (oversized ones cut into windows), up to the character limit.
    fn edit_units(&self, task: &Task, text: &str, plan: &Plan) -> Outcome {
        let segs = window::refine(chunk::split(text, task.unit), &|s| plan.fits(s), &|s| self.backend.count(s).is_some_and(|n| plan.fits_tokens(n)));
        let units = segs.iter().filter(|s| matches!(s, Seg::Unit(_))).count();
        let (mut answers, mut first_reason, mut kept, mut spent) = (Vec::new(), None, 0, 0);
        for unit in segs.iter().filter_map(|seg| if let Seg::Unit(unit) = seg { Some(*unit) } else { None }) {
            if self.cancel.is_set() {
                return Outcome::Cancelled;
            }
            if spent > 0 && spent + unit.chars().count() > self.total_chars {
                break;
            }
            spent += unit.chars().count();
            match self.ask_unit(task, unit, plan) {
                Ok(Answer::Used(better)) => answers.push(better),
                Ok(Answer::Kept(reason)) => {
                    crate::log::note(&format!("text answer rejected ({})", reason.code()));
                    first_reason.get_or_insert(reason);
                    kept += 1;
                    answers.push(unit.to_string());
                }
                Err(outcome) => return outcome,
            }
        }
        if let Some(reason) = first_reason.filter(|_| kept == answers.len()) {
            return Outcome::NoUsableAnswer(reason);
        }
        let notice = (answers.len() < units).then(|| reached(&segs, answers.len(), text.chars().count()));
        Outcome::Edited { text: chunk::join(&segs, &answers), notice }
    }

    /// A whole-text task: one request with as much of the start of the text as fits. The answer replaces the text.
    fn answer_whole(&self, task: &Task, text: &str, plan: &Plan) -> Outcome {
        let body = text.trim();
        let head = window::head(body, &|s| plan.fits(s) || self.backend.count(s).is_some_and(|n| plan.fits_tokens(n)));
        if head.is_empty() {
            return Outcome::Edited { text: text.to_string(), notice: None };
        }
        let notice = (head.len() < body.len()).then(|| format!("Used the first {} of {} characters; the rest was left out.", head.chars().count(), body.chars().count()));
        match self.ask_unit(task, head, plan) {
            Ok(Answer::Used(answer)) => Outcome::Edited { text: answer, notice },
            Ok(Answer::Kept(reason)) => Outcome::NoUsableAnswer(reason),
            Err(outcome) => outcome,
        }
    }

    /// One request and its judgement. Returns the answer to use or the reason to keep the unit as written; Err carries Cancelled or Failed, which end the edit.
    fn ask_unit(&self, task: &Task, unit: &str, plan: &Plan) -> Result<Answer, Outcome> {
        let clean = strip_hidden(unit);
        let masked = task.mask.then(|| mask::lift(&clean));
        let sent = masked.as_ref().map_or(clean.as_str(), |m| m.text());
        let raw = match self.backend.ask(task, sent, plan.max_tokens(sent), self.cancel) {
            Ok(raw) => raw,
            Err(Fail::Cancelled) => return Err(Outcome::Cancelled),
            Err(Fail::Failed(why)) => return Err(Outcome::Failed(why)),
        };
        let answer = match &masked {
            Some(m) => match m.restore(&raw) {
                Some(restored) => restored,
                None => return Ok(Answer::Kept(Reason::MaskBroken)),
            },
            None => raw,
        };
        Ok(match reply::accept_for(task, &clean, &answer) {
            Verdict::Accept(text) => Answer::Used(text),
            Verdict::Reject(reason) => Answer::Kept(reason),
        })
    }
}

/// The notice for an editing run that stopped early. Inputs: the segments, how many units were processed, the text's length in characters. Returns "Edited the first X of Y characters."
fn reached(segs: &[Seg], processed: usize, total: usize) -> String {
    let (mut chars, mut seen) = (0, 0);
    for seg in segs {
        let (Seg::Unit(piece) | Seg::Keep(piece)) = seg;
        if matches!(seg, Seg::Unit(_)) {
            if seen == processed {
                break;
            }
            seen += 1;
        }
        chars += piece.chars().count();
    }
    format!("Edited the first {chars} of {total} characters.")
}
