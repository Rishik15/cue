//! Purpose: Works out a note on request: the math result of each line. Needs no database, so it runs on the spot.
//! Contents: Evaluated — the answer; evaluate — text to an answer.

use serde::Serialize;

use super::{math, store, Reply};

/// Math results by line (None where a line shows nothing).
#[derive(Serialize)]
pub struct Evaluated {
    pub results: Vec<Option<String>>,
}

pub fn evaluate(text: &str, digits: u8, thousands: bool) -> Result<Reply, String> {
    if text.len() > store::MAX_BODY * 4 {
        return Err("The note is too long to work out.".into());
    }
    let look = math::Look { digits: usize::from(digits.min(7)), thousands };
    Ok(Reply::Evaluated(Evaluated { results: math::evaluate(text, &look) }))
}
