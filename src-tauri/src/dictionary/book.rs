//! File: dictionary/book.rs
//! Purpose: The dictionary as the window sees it: one small interface over the installed pack files. `lookup` answers one word with its full entry, `search` answers a typed prefix with a few headwords; both return plain data and nothing else about packs leaks out.
//! Contents: Hit — one prefix match; Entry — one word's blocks and the pack's language; Book — the installed pack files (`lookup`, `search`; a search keeps each pack's read-only connection until the Book is dropped); `fold_query` — fold a typed query; `clean_query` — validate and fold it; `successor` — the first string after every string that starts with a prefix.
//! The window's query is untrusted: `clean_query` bounds its length and characters before any SQL runs, and every statement binds its values.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusqlite::{params, Connection};
use serde::Serialize;

use super::pack::{self, Block};

/// The longest query accepted, in characters: a pack key is never longer (`pack::normalize`).
pub const MAX_QUERY: usize = 48;
/// The most matches one search returns, whatever the caller asks for.
pub const MAX_HITS: usize = 20;

/// One prefix match: the key it is looked up by, the word as written, its first part of speech and first meaning (enough for a result row and a preview).
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Hit {
    pub key: String,
    pub word: String,
    pub pos: String,
    pub gloss: String,
}

/// A word's full entry: its key, the language of the pack that has it, the id of that pack (kept for the Looked Up list, not sent to the page) and its blocks (one per part of speech).
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Entry {
    pub key: String,
    pub language: String,
    #[serde(skip)]
    pub pack: &'static str,
    pub blocks: Vec<Block>,
}

/// One installed pack: its id, its name (the language when the file does not say) and its file.
pub type PackFile = (&'static str, &'static str, PathBuf);

/// The installed pack files, in the order they are asked, and the read-only connections `search` has opened (kept so each keystroke reuses them and their prepared statement; dropping the Book closes them).
pub struct Book {
    packs: Vec<PackFile>,
    readers: Mutex<HashMap<PathBuf, Connection>>,
}

/// Folds a typed query as `src/lookup/nav.ts` does for the rows it keeps while a reply is out (the same cases in `src/lookup/fold-cases.json`). Input: the text. Return: it lower case, curly apostrophes straight, no leading white space, every run of white space one space (white space as `char::is_whitespace` has it).
pub fn fold_query(query: &str) -> String {
    let mut folded = String::new();
    for c in query.replace('\u{2019}', "'").to_lowercase().trim_start().chars() {
        if !c.is_whitespace() {
            folded.push(c);
        } else if !folded.ends_with(' ') {
            folded.push(' ');
        }
    }
    folded
}

/// Folds a typed query to the lower-case prefix it is searched under. Input: the text. Return: Ok(None) for an empty query, Err for one that is too long or holds anything but letters, combining marks, spaces, apostrophes and hyphens, else the prefix.
pub fn clean_query(query: &str) -> Result<Option<String>, String> {
    if query.chars().count() > MAX_QUERY {
        return Err("That is too long for a word.".into());
    }
    let folded = fold_query(query);
    if folded.is_empty() {
        return Ok(None);
    }
    let plain = folded.chars().all(|c| c.is_alphabetic() || matches!(c, '\u{300}'..='\u{36F}' | ' ' | '\'' | '-'));
    if plain {
        Ok(Some(folded))
    } else {
        Err("Type letters only.".into())
    }
}

/// The smallest string greater than every string that starts with `prefix`, in the byte order SQLite compares text in (which is code point order, the order of Rust's `str`). Input: a non-empty prefix. Return: None when no such string exists.
pub fn successor(prefix: &str) -> Option<String> {
    let mut chars: Vec<char> = prefix.chars().collect();
    while let Some(last) = chars.pop() {
        let next = (u32::from(last) + 1..=u32::from(char::MAX)).find_map(char::from_u32);
        if let Some(next) = next {
            chars.push(next);
            return Some(chars.into_iter().collect());
        }
    }
    None
}

impl Book {
    /// A book over the given pack files. Input: each pack's id, name and path.
    pub fn new(packs: Vec<PackFile>) -> Book {
        Book { packs, readers: Mutex::new(HashMap::new()) }
    }

    /// Whether no pack is installed.
    pub fn is_empty(&self) -> bool {
        self.packs.is_empty()
    }

    /// The entry of a word. Input: a selection or typed word; it is normalised first. Return: Ok(None) when it is not a word or no pack has it; the first pack that has it wins, and a pack that cannot be read is skipped unless no other pack has the word.
    pub fn lookup(&self, word: &str) -> Result<Option<Entry>, String> {
        let Some(key) = pack::normalize(word) else { return Ok(None) };
        let mut unreadable = None;
        for (id, name, path) in &self.packs {
            match pack::open(path).and_then(|conn| Ok((pack::define(&conn, &key)?, pack::language(&conn)))) {
                Ok((blocks, _)) if blocks.is_empty() => {}
                Ok((blocks, language)) => {
                    return Ok(Some(Entry { key, language: language.unwrap_or_else(|| (*name).into()), pack: id, blocks }));
                }
                Err(e) => unreadable = Some(e),
            }
        }
        unreadable.map_or(Ok(None), Err)
    }

    /// The words that start with a typed prefix, in alphabetical order. Input: the typed text and the most matches wanted (at most `MAX_HITS`). Return: the matches, empty for an empty query; Err for a query that fails `clean_query` or when no pack could be read.
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<Hit>, String> {
        let Some(prefix) = clean_query(query)? else { return Ok(Vec::new()) };
        let Some(above) = successor(&prefix) else { return Ok(Vec::new()) };
        let limit = limit.min(MAX_HITS);
        let (mut hits, mut failure) = (Vec::new(), None);
        for (_, _, path) in &self.packs {
            match self.with_reader(path, |conn| prefix_hits(conn, &prefix, &above, limit)) {
                Ok(found) => hits.extend(found),
                Err(e) => failure = Some(e),
            }
        }
        if hits.is_empty() && failure.is_some() {
            return Err(failure.unwrap_or_default());
        }
        hits.sort_by(|a, b| a.key.cmp(&b.key));
        hits.dedup_by(|a, b| a.key == b.key);
        hits.truncate(limit);
        Ok(hits)
    }

    /// Runs a read on one pack's connection, opening it the first time. Input: the pack file and the read. Return: the read's result; Err when the pack cannot be opened or the read fails.
    fn with_reader<T>(&self, path: &Path, read: impl FnOnce(&Connection) -> Result<T, String>) -> Result<T, String> {
        let mut readers = self.readers.lock().unwrap_or_else(|e| e.into_inner());
        if !readers.contains_key(path) {
            readers.insert(path.to_path_buf(), pack::open(path)?);
        }
        read(&readers[path])
    }
}

/// The first `limit` entries of one pack whose key starts with `prefix` (`key >= prefix AND key < above` is a range read on the key index); rows are read until `limit` are valid or the range ends, so an entry whose data is damaged or empty is left out without shortening the answer.
fn prefix_hits(conn: &Connection, prefix: &str, above: &str, limit: usize) -> Result<Vec<Hit>, String> {
    let read = |e: rusqlite::Error| format!("Could not read the dictionary: {e}");
    let mut statement = conn.prepare_cached("SELECT key, data FROM entries WHERE key >= ?1 AND key < ?2 ORDER BY key").map_err(read)?;
    let rows = statement.query_map(params![prefix, above], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))).map_err(read)?;
    let mut hits = Vec::new();
    for row in rows {
        let (key, data) = row.map_err(read)?;
        hits.extend(hit_of(key, &data));
        if hits.len() >= limit {
            break;
        }
    }
    Ok(hits)
}

/// The result row of an entry's JSON: its first block and first meaning; None when the data is damaged or has neither.
fn hit_of(key: String, data: &str) -> Option<Hit> {
    let blocks: Vec<Block> = serde_json::from_str(data).ok()?;
    let block = blocks.first()?;
    let gloss = block.senses.first()?.gloss.clone();
    Some(Hit { key, word: block.word.clone(), pos: block.pos.clone(), gloss })
}
