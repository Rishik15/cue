//! Purpose: The words the user saved from the popup: a small SQLite file in the app data folder, opened for one operation and closed again.
//! Contents: Saved — one row as the Saved Words page shows it; open — the file; save / list / remove / is_saved — the writes and reads (the commands are in `mod.rs`).
//! Bound parameters only. A word is unique per pack, so saving twice keeps one row.

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
    pub ipa: String,
    pub gloss: String,
    pub example: String,
    pub language: String,
    pub added: i64,
}

pub fn open(path: &Path) -> Result<Connection, String> {
    let conn = Connection::open(path).map_err(|e| format!("Could not open the saved words: {e}"))?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS words (id INTEGER PRIMARY KEY, key TEXT NOT NULL, pack TEXT NOT NULL, word TEXT NOT NULL, pos TEXT NOT NULL,
            ipa TEXT NOT NULL, gloss TEXT NOT NULL, example TEXT NOT NULL, language TEXT NOT NULL, added INTEGER NOT NULL, UNIQUE (pack, key))",
    )
    .map_err(|e| format!("Could not prepare the saved words: {e}"))?;
    Ok(conn)
}

pub fn save(conn: &Connection, pack: &str, key: &str, word: &Saved) -> Result<(), String> {
    conn.execute(
        "INSERT OR IGNORE INTO words (key, pack, word, pos, ipa, gloss, example, language, added) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![key, pack, word.word, word.pos, word.ipa, word.gloss, word.example, word.language, word.added],
    )
    .map(|_| ())
    .map_err(|e| format!("Could not save the word: {e}"))
}

pub fn is_saved(conn: &Connection, pack: &str, key: &str) -> bool {
    conn.query_row("SELECT 1 FROM words WHERE pack = ?1 AND key = ?2", params![pack, key], |_| Ok(())).is_ok()
}

/// Newest first.
pub fn list(conn: &Connection) -> Result<Vec<Saved>, String> {
    let mut statement = conn
        .prepare("SELECT id, word, pos, ipa, gloss, example, language, added FROM words ORDER BY added DESC, id DESC")
        .map_err(|e| format!("Could not read the saved words: {e}"))?;
    let rows = statement
        .query_map([], |r| {
            Ok(Saved {
                id: r.get(0)?,
                word: r.get(1)?,
                pos: r.get(2)?,
                ipa: r.get(3)?,
                gloss: r.get(4)?,
                example: r.get(5)?,
                language: r.get(6)?,
                added: r.get(7)?,
            })
        })
        .map_err(|e| format!("Could not read the saved words: {e}"))?;
    rows.collect::<Result<_, _>>().map_err(|e| format!("Could not read the saved words: {e}"))
}

pub fn remove(conn: &Connection, id: i64) -> Result<(), String> {
    conn.execute("DELETE FROM words WHERE id = ?1", [id]).map(|_| ()).map_err(|e| format!("Could not remove the word: {e}"))
}
