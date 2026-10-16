//! Purpose: Read a dictionary pack (a SQLite file built by `scripts/build-dictionary.mjs`) and turn a selection into the entries the popup shows.
//! Contents: normalize — a selection to the lower-case key it is looked up under (a word or a short phrase, else None); Block / Sense — one part of speech of a word and its meanings;
//! open — the pack file, read only; define — the blocks for a word, found through its base form when it is inflected ("reached" finds "reach"); language — the pack's language name.
//! A pack holds `entries(key, data)` (the lower-case headword and a JSON array of blocks) and `forms(form, key)` for irregular inflections. Only keyed reads, one per lookup, so nothing is loaded in advance.

use std::path::Path;

use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde::{Deserialize, Serialize};

#[cfg(test)]
#[path = "pack_test.rs"]
mod tests;

const MAX_WORDS: usize = 4;
const MAX_CHARS: usize = 48;
/// Endings WordNet's own lookup strips to find a base word: the ending, what replaces it ("ies" becomes "y"), and the part of speech the ending suggests (shown first).
/// Irregular forms are in the pack's `forms` table.
const ENDINGS: [(&str, &str, &str); 18] = [
    ("s", "", ""),
    ("ses", "s", ""),
    ("xes", "x", ""),
    ("zes", "z", ""),
    ("ches", "ch", ""),
    ("shes", "sh", ""),
    ("men", "man", "noun"),
    ("ies", "y", ""),
    ("es", "e", ""),
    ("es", "", ""),
    ("ed", "e", "verb"),
    ("ed", "", "verb"),
    ("ing", "e", "verb"),
    ("ing", "", "verb"),
    ("er", "", "adjective"),
    ("est", "", "adjective"),
    ("er", "e", "adjective"),
    ("est", "e", "adjective"),
];

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
    #[serde(alias = "i", default, skip_serializing_if = "Option::is_none")]
    pub ipa: Option<String>,
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

/// A base word an inflected form may come from, and the part of speech to show first ("" for none).
type Stem = (String, &'static str);

/// Base words for a form: without a possessive ending, then with each ending replaced ("stopped" tries "stopp", "stope" and "stop").
fn stems(key: &str) -> Vec<Stem> {
    let mut all: Vec<Stem> = Vec::new();
    if let Some(base) = key.strip_suffix("'s").or_else(|| key.strip_suffix("s'")) {
        all.push((base.to_string(), ""));
    }
    for (ending, with, pos) in ENDINGS {
        let Some(stem) = key.strip_suffix(ending).filter(|s| s.chars().count() >= 2) else { continue };
        let stem = format!("{stem}{with}");
        // A doubled final consonant came from the inflection ("stopp" from "stopped").
        let chars: Vec<char> = stem.chars().collect();
        if let [.., a, b] = chars[..] {
            if a == b && !"aeiou".contains(a) {
                all.push((chars[..chars.len() - 1].iter().collect(), pos));
            }
        }
        all.push((stem, pos));
    }
    all
}

pub fn open(path: &Path) -> Result<Connection, String> {
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)
        .map_err(|e| format!("Could not open the dictionary: {e}"))
}

pub fn language(conn: &Connection) -> Option<String> {
    conn.query_row("SELECT v FROM meta WHERE k = 'language'", [], |r| r.get(0)).ok()
}

fn blocks(conn: &Connection, key: &str) -> Result<Vec<Block>, String> {
    let data: Option<String> = conn
        .query_row("SELECT data FROM entries WHERE key = ?1", [key], |r| r.get(0))
        .optional()
        .map_err(|e| format!("Could not read the dictionary: {e}"))?;
    data.map_or(Ok(Vec::new()), |json| serde_json::from_str(&json).map_err(|e| format!("The dictionary file is damaged: {e}")))
}

fn irregular(conn: &Connection, form: &str) -> Option<(String, String)> {
    conn.query_row("SELECT key, pos FROM forms WHERE form = ?1", [form], |r| Ok((r.get(0)?, r.get(1)?))).ok()
}

/// The entries for a word, empty when there are none: the word itself, then its inflected form's base ("went" is "go"), then the bases its ending suggests.
/// The part of speech the inflection belongs to comes first ("went" is a verb form).
pub fn define(conn: &Connection, key: &str) -> Result<Vec<Block>, String> {
    let mut candidates = vec![(key.to_string(), String::new())];
    candidates.extend(irregular(conn, key));
    candidates.extend(stems(key).into_iter().map(|(stem, pos)| (stem, pos.to_string())));
    for (candidate, pos) in candidates {
        let mut found = blocks(conn, &candidate)?;
        if !found.is_empty() {
            found.sort_by_key(|b| b.pos != pos); // stable: the rest keep their order
            return Ok(found);
        }
    }
    Ok(Vec::new())
}
