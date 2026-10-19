//! Purpose: Works out a note on request: the math result of each line and the counts for the status line. Needs no database, so it runs on the spot.
//! Contents: Evaluated — the answer; evaluate — text and optional selection to an answer.

use serde::Serialize;

use super::{math, stats, store, Reply};

/// Math results by line (None where a line shows nothing) and the counts the status line shows.
#[derive(Serialize)]
pub struct Evaluated {
    pub results: Vec<Option<String>>,
    stats: Option<stats::Stats>,
}

/// Needs no database, so it runs on the spot.
pub fn evaluate(text: &str, selection: Option<&str>, digits: u8, thousands: bool) -> Result<Reply, String> {
    if text.len() > store::MAX_BODY * 4 {
        return Err("The note is too long to work out.".into());
    }
    let selection = selection.filter(|s| s.len() <= store::MAX_BODY * 4);
    let look = math::Look { digits: usize::from(digits.min(7)), thousands };
    let first = text.split('\n').next().unwrap_or("");
    let stats = match selection.filter(|s| !s.trim().is_empty()) {
        Some(selection) => Some(stats::of(selection, &look)),
        None if stats::keyword(first) => Some(stats::of(text.split_once('\n').map_or("", |(_, rest)| rest), &look)),
        None => None,
    };
    Ok(Reply::Evaluated(Evaluated { results: math::evaluate(text, &look), stats }))
}
