//! File: lab/clean.rs
//! Purpose: `cue --lab clean <file>`: strip hidden characters from lab inputs with production's own `strip_hidden`, so the lab never mirrors Cue's hidden-character list.
//! Contents: clean_strings — a JSON array of strings in, the same array cleaned out.

use crate::sanitize::strip_hidden;

/// Cleans every string as the text pipeline cleans a unit before the model sees it. Input: a JSON array of strings. Returns the cleaned array as JSON, or an error for invalid JSON.
pub fn clean_strings(json: &str) -> Result<String, String> {
    let texts: Vec<String> = serde_json::from_str(json).map_err(|e| format!("the inputs are not a JSON array of strings: {e}"))?;
    let cleaned: Vec<String> = texts.iter().map(|t| strip_hidden(t)).collect();
    serde_json::to_string(&cleaned).map_err(|e| format!("cannot write the cleaned inputs: {e}"))
}
