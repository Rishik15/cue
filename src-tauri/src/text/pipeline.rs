//! File: text/pipeline.rs
//! Purpose: The edit of a whole text as one call: size each request to the model's context, cut oversized units into windows, ask a backend for each, keep only answers the checks trust, and join the result. Knows nothing about processes or HTTP.
//! Contents: Outcome — Edited (with an optional notice), NoUsableAnswer (with the reason), Cancelled or Failed; Pipeline — `run` (editing: the unit loop; whole-text tasks: the head of the text in one request) and `warm` (load the model and its prompt prefix ahead of a request).
//! A unit whose answer was cut at the token limit is kept as written (Runaway, judged on its own); a whole-text answer that was cut is used and the notice says so.
//! Each unit is sent without hidden characters (tag characters can smuggle instructions); a rejected answer leaves that unit as written, so one bad answer never costs the rest. Nothing is refused for being long: editing stops after `total_chars` and says so, a whole-text task uses the head that fits and says so.

use super::backend::{Backend, Reply};
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

/// The notice added to a whole-text answer that the model cut off at its length limit.
const CUT_NOTICE: &str = "The answer was cut at the length limit.";

/// What became of one unit: its answer was used, was used but cut off at the token limit, or the original stays and this is why.
enum Answer {
    Used(String),
    Cut(String),
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

    /// The editing loop: the units in order (oversized ones cut into windows), up to the character limit. Only the start of a long text is split at all: what lies beyond the limit and the widest window is never looked at and comes back as written.
    fn edit_units(&self, task: &Task, text: &str, plan: &Plan) -> Outcome {
        let reachable = window::prefix(text, self.total_chars + plan.reach());
        let capped = reachable.len() < text.len();
        let mut segs = chunk::split(reachable, task.unit);
        if capped && matches!(segs.last(), Some(Seg::Unit(_))) {
            segs.pop();
        }
        let covered: usize = segs
            .iter()
            .map(|seg| match seg {
                Seg::Unit(piece) | Seg::Keep(piece) => piece.len(),
            })
            .sum();
        let segs = window::refine(segs, &|s| plan.fits(s), &|s| self.backend.count(s).is_some_and(|n| plan.fits_tokens(n)), self.cancel);
        if self.cancel.is_set() {
            return Outcome::Cancelled;
        }
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
            let rejected = match self.ask_unit(task, unit, plan) {
                Ok(Answer::Used(better)) => {
                    answers.push(better);
                    None
                }
                Ok(Answer::Cut(_)) => Some(Reason::Runaway),
                Ok(Answer::Kept(reason)) => Some(reason),
                Err(outcome) => return outcome,
            };
            if let Some(reason) = rejected {
                crate::log::note(&format!("text answer rejected ({})", reason.code()));
                first_reason.get_or_insert(reason);
                kept += 1;
                answers.push(unit.to_string());
            }
        }
        if let Some(reason) = first_reason.filter(|_| kept == answers.len()) {
            return Outcome::NoUsableAnswer(reason);
        }
        let notice = (capped || answers.len() < units).then(|| reached(&segs, answers.len(), text.chars().count()));
        Outcome::Edited { text: chunk::join(&segs, &answers) + &text[covered..], notice }
    }

    /// A whole-text task: one request with as much of the start of the text as fits. The answer replaces the text. An answer cut off at the length limit is used and the notice says so:
    /// a long explanation or reply that is cut is still worth reading, and the user sees it before it replaces anything, whereas refusing would fail every text whose answer runs long.
    fn answer_whole(&self, task: &Task, text: &str, plan: &Plan) -> Outcome {
        let body = text.trim();
        let reachable = window::prefix(body, plan.reach());
        let head = window::head(reachable, &|s| plan.fits(s) || self.backend.count(s).is_some_and(|n| plan.fits_tokens(n)), self.cancel);
        if self.cancel.is_set() {
            return Outcome::Cancelled;
        }
        if head.is_empty() {
            return Outcome::Edited { text: text.to_string(), notice: None };
        }
        let left_out = (head.len() < body.len())
            .then(|| format!("Used the first {} of {} characters; the rest was left out.", head.chars().count(), body.chars().count()));
        match self.ask_unit(task, head, plan) {
            Ok(Answer::Used(answer)) => Outcome::Edited { text: answer, notice: left_out },
            Ok(Answer::Cut(answer)) => Outcome::Edited { text: answer, notice: join_notices(left_out, CUT_NOTICE) },
            Ok(Answer::Kept(reason)) => Outcome::NoUsableAnswer(reason),
            Err(outcome) => outcome,
        }
    }

    /// One request and its judgement. Returns the answer to use or the reason to keep the unit as written; Err carries Cancelled or Failed, which end the edit.
    fn ask_unit(&self, task: &Task, unit: &str, plan: &Plan) -> Result<Answer, Outcome> {
        let clean = strip_hidden(unit);
        let masked = task.mask.then(|| mask::lift(&clean));
        let sent = masked.as_ref().map_or(clean.as_str(), |m| m.text());
        let Reply { text: raw, cut } = match self.backend.ask(task, sent, plan.max_tokens(sent), self.cancel) {
            Ok(reply) => reply,
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
            Verdict::Accept(text) if cut => Answer::Cut(text),
            Verdict::Accept(text) => Answer::Used(text),
            Verdict::Reject(reason) => Answer::Kept(reason),
        })
    }
}

/// Joins the notice about text left out (if any) with the notice about a cut answer. Inputs: the first notice and the second. Returns one notice.
fn join_notices(first: Option<String>, second: &str) -> Option<String> {
    Some(first.map_or_else(|| second.to_string(), |first| format!("{first} {second}")))
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
