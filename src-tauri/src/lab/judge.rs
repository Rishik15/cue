//! File: lab/judge.rs
//! Purpose: `cue --lab judge <file>`: judge model answers with production's reply check, so the lab scores what Cue would accept and never mirrors the check in PowerShell.
//! Contents: judge_rows — a JSON array of {action, input, answer} in, one {accepted, reason, text} per row out; judge_row — one row.

use serde::{Deserialize, Serialize};

use crate::actions::{task, Action};
use crate::text::reply::{accept_for, strip_wrapping, Verdict};

/// One answer to judge: the action id, the input as sent (already cleaned of hidden characters) and the model's raw answer.
#[derive(Deserialize)]
struct Row {
    action: String,
    input: String,
    answer: String,
}

/// The verdict on one answer: whether production accepts it, the rejection code (None when accepted) and the tidied answer either way.
#[derive(Serialize)]
struct Judged {
    accepted: bool,
    reason: Option<&'static str>,
    text: String,
}

/// Judges every row the way the text pipeline judges a unit. Input: the JSON array. Returns the JSON array of verdicts in the same order, or an error for invalid JSON or an action without a model task.
pub fn judge_rows(json: &str) -> Result<String, String> {
    let rows: Vec<Row> =
        serde_json::from_str(json).map_err(|e| format!("the rows are not a JSON array of {{action, input, answer}}: {e}"))?;
    let judged = rows.iter().map(judge_row).collect::<Result<Vec<_>, _>>()?;
    serde_json::to_string(&judged).map_err(|e| format!("cannot write the verdicts: {e}"))
}

/// Judges one row with `accept_for`. Input: the row. Returns its verdict, or an error naming an unknown or exact action.
fn judge_row(row: &Row) -> Result<Judged, String> {
    let task =
        Action::all().find(|a| a.id() == row.action).and_then(task).ok_or_else(|| format!("{:?} is not a model action", row.action))?;
    Ok(match accept_for(task, &row.input, &row.answer) {
        Verdict::Accept(text) => Judged { accepted: true, reason: None, text },
        Verdict::Reject(reason) => Judged { accepted: false, reason: Some(reason.code()), text: strip_wrapping(&row.answer, &row.input) },
    })
}
