//! File: text/budget.rs
//! Purpose: How much text one request may carry, measured in tokens: the model's context minus the task's fixed prefix, a margin for the chat template, and the room the answer needs. Characters are only the fallback estimate when the server cannot count.
//! Contents: Plan — the budget of one task on one model (`new`, `fits`, `reach`, `max_tokens`); estimate — a safe upper estimate of a text's tokens; prefix_text — the fixed part of a task's prompt; EDIT_TOTAL_CHARS and the margins.
//! The numbers are unmeasured: EDIT_TOTAL_CHARS is a first guess that the bench (latency per 500, 2,000 and 6,000 characters per model) must confirm or change.

use super::reply;
use crate::actions::{Scope, Task};

/// Tokens kept free for the chat template and role markers around every request.
pub const MARGIN: usize = 64;

/// Longest answer asked of a whole-text task (summary, explain, reply, action items), in tokens.
pub const WHOLE_OUTPUT: usize = 384;

/// Most characters an editing action works through in one run; the rest of the text stays as written and the user is told. Unmeasured first guess (about 50 sentences, some seconds warm).
pub const EDIT_TOTAL_CHARS: usize = 6_000;

/// Most characters one token is taken to hold when sizing how far into a text a window can reach; a generous bound, since a window longer than this is only ever cut shorter, never wrong.
const REACH_CHARS_PER_TOKEN: usize = 8;

/// A safe upper estimate of the tokens in `text`: three ASCII characters to a token, one token per other character. Used to decide cheaply, and when the server cannot count.
pub fn estimate(text: &str) -> usize {
    let ascii = text.chars().filter(char::is_ascii).count();
    ascii.div_ceil(3) + (text.chars().count() - ascii)
}

/// The fixed part of a task's prompt: instruction and examples, as one text to count.
pub fn prefix_text(task: &Task) -> String {
    let mut text = task.system.to_string();
    for (said, answered) in task.shots {
        text.push('\n');
        text.push_str(said);
        text.push('\n');
        text.push_str(answered);
    }
    text
}

/// The token budget of one task on one model.
pub struct Plan {
    context: usize,
    prefix: usize,
    input: usize,
    output: usize,
}

impl Plan {
    /// Inputs: the model's context in tokens, the tokens of the task's fixed prefix and the task's scope. An editing request gets half of what is left for its text (the answer is about as long as the text);
    /// a whole-text request gets all but `WHOLE_OUTPUT`. Returns the plan.
    pub fn new(context: usize, prefix: usize, scope: Scope) -> Self {
        let room = context.saturating_sub(prefix + MARGIN);
        match scope {
            Scope::Units => Self { context, prefix, input: room / 2, output: usize::MAX },
            Scope::Whole => {
                let output = WHOLE_OUTPUT.min(room / 2);
                Self { context, prefix, input: room - output, output }
            }
        }
    }

    /// Whether `text` surely fits one request, by the safe estimate.
    pub fn fits(&self, text: &str) -> bool {
        estimate(text) <= self.input
    }

    /// The most characters of a text that can matter to one window: its input room at the most characters a token holds, at least one token's worth. Returns the character count.
    pub fn reach(&self) -> usize {
        self.input.max(1) * REACH_CHARS_PER_TOKEN
    }

    /// Whether a token count fits one request.
    pub fn fits_tokens(&self, tokens: usize) -> bool {
        tokens <= self.input
    }

    /// The most tokens to ask the model to write for `sent`: the usual allowance for the text's length, never more than the room left in the context and never more than the task's output cap.
    pub fn max_tokens(&self, sent: &str) -> usize {
        let left = self.context.saturating_sub(self.prefix + MARGIN + estimate(sent));
        reply::max_tokens(sent).min(left).min(self.output).max(16)
    }
}
