//! Purpose: Works out a note on request: the math result of each line. Needs no database, so it runs on the spot.
//! Contents: Evaluated — the answer (results, and the lines that are a calculation waiting for their `=`); evaluate — text to an answer.

use serde::Serialize;

use super::{math, store, Reply};

/// Math results by line (None where a line shows nothing), and per line whether it is a calculation that only lacks its `=`.
#[derive(Serialize)]
pub struct Evaluated {
    pub results: Vec<Option<String>>,
    pub ready: Vec<bool>,
    /// The value each line gave to the name it defines.
    pub values: Vec<Option<String>>,
}

/// The answer for a note's text with the note settings' look (also what the window is built with, so the first paint already has its results).
pub fn answers(text: &str, digits: u8, thousands: bool) -> Evaluated {
    let look = math::Look { digits: usize::from(digits.min(7)), thousands };
    let lines = math::evaluate(text, &look);
    Evaluated {
        results: lines.iter().map(|l| l.shown.clone()).collect(),
        ready: lines.iter().map(|l| l.ready).collect(),
        values: lines.into_iter().map(|l| l.value).collect(),
    }
}

pub fn evaluate(text: &str, digits: u8, thousands: bool) -> Result<Reply, String> {
    if text.len() > store::MAX_BODY * 4 {
        return Err("The note is too long to work out.".into());
    }
    Ok(Reply::Evaluated(answers(text, digits, thousands)))
}
