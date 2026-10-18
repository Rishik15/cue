//! Purpose: Read a dictionary pack (a SQLite file built by `scripts/build-dictionary.mjs`) and turn a selection into the entries the popup shows.
//! Contents: normalize — a selection to the lower-case key it is looked up under (a word or a short phrase, else None); Block / Sense — one part of speech of a word and its meanings;
//! open — the pack file, read only; define — the blocks for a word; language — the pack's language name.
//! A pack holds `entries(key, data)`: the lower-case word and a JSON array of blocks. Inflected forms are entries of their own ("went": "past of go"), so a lookup is one keyed read and nothing is loaded in advance.

use std::path::Path;

use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde::{Deserialize, Serialize};

#[cfg(test)]
#[path = "pack_test.rs"]
mod tests;

const MAX_WORDS: usize = 4;
const MAX_CHARS: usize = 48;

/// One meaning: its gloss and, when the dictionary has one, a sentence that uses it. The pack stores short keys (`g`, `e`) to stay small.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct Sense {
    #[serde(alias = "g")]
    pub gloss: String,
    #[serde(alias = "e", default, skip_serializing_if = "Option::is_none")]
    pub example: Option<String>,
}

/// One part of speech of one word.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct Block {
    #[serde(alias = "w")]
    pub word: String,
    #[serde(alias = "p")]
    pub pos: String,
    #[serde(alias = "s")]
    pub senses: Vec<Sense>,
    #[serde(alias = "y", default, skip_serializing_if = "Vec::is_empty")]
    pub synonyms: Vec<String>,
}

/// The key a selection is looked up under, or None when it is not a word or a short phrase (digits, symbols, a sentence).
pub fn normalize(selection: &str) -> Option<String> {
    let text = selection.replace('\u{2019}', "'");
    let trimmed = text.trim_matches(|c: char| !c.is_alphanumeric());
    let key = trimmed.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
    let words = key.split(' ').count();
    let plain = key.chars().all(|c| c.is_alphabetic() || matches!(c, ' ' | '\'' | '-'));
    (!key.is_empty() && words <= MAX_WORDS && key.chars().count() <= MAX_CHARS && plain).then_some(key)
}

pub fn open(path: &Path) -> Result<Connection, String> {
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)
        .map_err(|e| format!("Could not open the dictionary: {e}"))
}

pub fn language(conn: &Connection) -> Option<String> {
    conn.query_row("SELECT v FROM meta WHERE k = 'language'", [], |r| r.get(0)).ok()
}

/// The entries for a word, empty when there are none.
pub fn define(conn: &Connection, key: &str) -> Result<Vec<Block>, String> {
    let data: Option<String> = conn
        .query_row("SELECT data FROM entries WHERE key = ?1", [key], |r| r.get(0))
        .optional()
        .map_err(|e| format!("Could not read the dictionary: {e}"))?;
    data.map_or(Ok(Vec::new()), |json| serde_json::from_str(&json).map_err(|e| format!("The dictionary file is damaged: {e}")))
}
