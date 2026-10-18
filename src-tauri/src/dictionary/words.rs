//! Purpose: The words the user looked up in the popup: a small SQLite file in the app data folder, opened for one operation and closed again.
//! Contents: Saved — one row as the Looked Up list shows it; open — the file; record / list / remove — the writes and reads (the commands are in `saved.rs`).
//! Bound parameters only. The `ipa` column stays in the table for files made by earlier versions; it is no longer read or written. A word is unique per pack: looking it up again moves its row to the top. Only the newest `KEEP` rows are kept.

use std::path::Path;

use rusqlite::{params, Connection};
use serde::Serialize;

#[cfg(test)]
#[path = "words_test.rs"]
mod tests;

#[derive(Serialize, Debug, PartialEq)]
pub struct Saved {
    pub id: i64,
    pub word: String,
    pub pos: String,
    pub gloss: String,
    pub example: String,
    pub language: String,
    pub added: i64,
}

pub fn open(path: &Path) -> Result<Connection, String> {
    let conn = Connection::open(path).map_err(|e| format!("Could not open the lookups: {e}"))?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS words (id INTEGER PRIMARY KEY, key TEXT NOT NULL, pack TEXT NOT NULL, word TEXT NOT NULL, pos TEXT NOT NULL,
            ipa TEXT NOT NULL DEFAULT '', gloss TEXT NOT NULL, example TEXT NOT NULL, language TEXT NOT NULL, added INTEGER NOT NULL, UNIQUE (pack, key))",
    )
    .map_err(|e| format!("Could not prepare the lookups: {e}"))?;
    Ok(conn)
}

/// The newest lookups kept; older rows are dropped as new ones arrive.
const KEEP: i64 = 500;

pub fn record(conn: &Connection, pack: &str, key: &str, word: &Saved) -> Result<(), String> {
    conn.execute(
        "INSERT INTO words (key, pack, word, pos, ipa, gloss, example, language, added) VALUES (?1, ?2, ?3, ?4, '', ?5, ?6, ?7, ?8)
         ON CONFLICT (pack, key) DO UPDATE SET word = ?3, pos = ?4, gloss = ?5, example = ?6, language = ?7, added = ?8",
        params![key, pack, word.word, word.pos, word.gloss, word.example, word.language, word.added],
    )
    .and_then(|_| conn.execute("DELETE FROM words WHERE id NOT IN (SELECT id FROM words ORDER BY added DESC, id DESC LIMIT ?1)", [KEEP]))
    .map(|_| ())
    .map_err(|e| format!("Could not record the word: {e}"))
}

/// Newest first.
pub fn list(conn: &Connection) -> Result<Vec<Saved>, String> {
    let mut statement = conn
        .prepare("SELECT id, word, pos, gloss, example, language, added FROM words ORDER BY added DESC, id DESC")
        .map_err(|e| format!("Could not read the lookups: {e}"))?;
    let rows = statement
        .query_map([], |r| {
            Ok(Saved {
                id: r.get(0)?,
                word: r.get(1)?,
                pos: r.get(2)?,
                gloss: r.get(3)?,
                example: r.get(4)?,
                language: r.get(5)?,
                added: r.get(6)?,
            })
        })
        .map_err(|e| format!("Could not read the lookups: {e}"))?;
    rows.collect::<Result<_, _>>().map_err(|e| format!("Could not read the lookups: {e}"))
}

pub fn remove(conn: &Connection, id: i64) -> Result<(), String> {
    conn.execute("DELETE FROM words WHERE id = ?1", [id]).map(|_| ()).map_err(|e| format!("Could not remove the word: {e}"))
}
