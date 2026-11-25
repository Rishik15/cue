//! File: dictionary/words.rs
//! Purpose: Looked Up, the words the user has seen an Entry for: a small SQLite file in the app data folder, opened for one operation and closed again.
//! Contents: Saved — one row as the Looked Up list shows it; open — the file; record / list / recent / remove — the writes and reads (the commands are in `saved.rs`).
//! Bound parameters only. The `ipa` column stays in the table for files made by earlier versions; it is no longer read or written. A word is unique per pack: looking it up again deletes its row and inserts a new one, so every read orders by the row id alone (`added` is only the time the page shows). Only the newest `KEEP` rows are kept.

use std::path::Path;

use rusqlite::{params, Connection};
use serde::Serialize;

/// One word of Looked Up: its row id, the word, its first part of speech and meaning, the Pack's language and when it was looked up (Unix seconds).
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

/// Opens (and creates) the Looked Up file. Input: its path. Return: the connection; Err says what failed.
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

/// Adds a word; when its Pack and key are already listed the old row is replaced by a new, newest one in the same transaction. Rows beyond `KEEP` are dropped. Input: the connection, the Pack id, the key and the row. Return: Ok, or Err when it cannot be written.
pub fn record(conn: &mut Connection, pack: &str, key: &str, word: &Saved) -> Result<(), String> {
    let write = |conn: &mut Connection| -> rusqlite::Result<()> {
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM words WHERE pack = ?1 AND key = ?2", [pack, key])?;
        tx.execute(
            "INSERT INTO words (key, pack, word, pos, gloss, example, language, added) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![key, pack, word.word, word.pos, word.gloss, word.example, word.language, word.added],
        )?;
        tx.execute("DELETE FROM words WHERE id NOT IN (SELECT id FROM words ORDER BY id DESC LIMIT ?1)", [KEEP])?;
        tx.commit()
    };
    write(conn).map_err(|e| format!("Could not record the word: {e}"))
}

/// Every word, newest first. Input: the connection. Return: the rows; Err when they cannot be read.
pub fn list(conn: &Connection) -> Result<Vec<Saved>, String> {
    let mut statement = conn
        .prepare("SELECT id, word, pos, gloss, example, language, added FROM words ORDER BY id DESC")
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

/// The newest words, newest first, reading only as many rows as are wanted. Input: the connection and how many. Return: each word with its first part of speech; Err when they cannot be read.
pub fn recent(conn: &Connection, limit: usize) -> Result<Vec<(String, String)>, String> {
    let read = |e: rusqlite::Error| format!("Could not read the lookups: {e}");
    let mut statement = conn.prepare("SELECT word, pos FROM words ORDER BY id DESC LIMIT ?1").map_err(read)?;
    let rows = statement.query_map([limit as i64], |r| Ok((r.get(0)?, r.get(1)?))).map_err(read)?;
    rows.collect::<Result<_, _>>().map_err(read)
}

/// Removes one word. Input: the connection and the row id. Return: Ok, or Err when it cannot be written.
pub fn remove(conn: &Connection, id: i64) -> Result<(), String> {
    conn.execute("DELETE FROM words WHERE id = ?1", [id]).map(|_| ()).map_err(|e| format!("Could not remove the word: {e}"))
}
