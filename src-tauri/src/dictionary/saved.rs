//! File: dictionary/saved.rs
//! Purpose: Looked Up and what uses it: the connection, the commands of the Dictionary page's Looked Up list, recording an Entry the user saw (a Card, or one opened in the Search window) and the Recent words the Search window lists.
//! Contents: words_conn — open (and create) `words.db` in the data folder; list_words / delete_word — commands for the page; record — add an Entry to Looked Up; Recent / recent — the newest words for the Search window.

use serde::Serialize;
use tauri::{AppHandle, Manager};

use super::book::Entry;
use super::words;

/// Opens the saved-words database, creating the data folder and the file on first use. Input: the app. Return: the connection; Err says what could not be created.
pub(super) fn words_conn(app: &AppHandle) -> Result<rusqlite::Connection, String> {
    let dir = app.path().app_data_dir().map_err(|e| format!("No place to keep Looked Up: {e}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("Could not create the data folder: {e}"))?;
    words::open(&dir.join("words.db"))
}

/// The Looked Up list. Input: the app. Return: every saved word, newest first; Err when the database cannot be read.
#[tauri::command]
pub async fn list_words(app: AppHandle) -> Result<Vec<words::Saved>, String> {
    crate::snippets::blocking(move || words::list(&words_conn(&app)?)).await
}

/// Removes one saved word. Input: the app and the row's id. Return: Ok, or Err when the database cannot be written.
#[tauri::command]
pub async fn delete_word(app: AppHandle, id: i64) -> Result<(), String> {
    crate::snippets::blocking(move || words::remove(&words_conn(&app)?, id)).await
}

/// Writes a lookup to the Looked Up list (first meaning only). A failure is only logged: the window must show whatever happens to the list. The Dictionary page reloads on the `words-changed` event this sends instead of on every window focus. Input: the app and the entry that was shown. Return: none.
pub(super) fn record(app: &AppHandle, entry: &Entry) {
    let Some(block) = entry.blocks.first() else { return };
    let Some(sense) = block.senses.first() else { return };
    let row = words::Saved {
        id: 0,
        word: block.word.clone(),
        pos: block.pos.clone(),
        gloss: sense.gloss.clone(),
        example: sense.example.clone().unwrap_or_default(),
        language: entry.language.clone(),
        added: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64),
    };
    if let Err(e) = words_conn(app).and_then(|conn| words::record(&conn, entry.pack, &entry.key, &row)) {
        crate::log::note(&format!("dictionary: {e}"));
        return;
    }
    let _ = tauri::Emitter::emit(app, "words-changed", ());
}

/// A word looked up before, as the search window lists it.
#[derive(Serialize, Debug, PartialEq)]
pub struct Recent {
    pub word: String,
    pub pos: String,
}

/// The newest looked-up words, newest first. Input: the app and how many. Return: the words; empty when the list cannot be read (the window opens without it).
pub(super) fn recent(app: &AppHandle, limit: usize) -> Vec<Recent> {
    let Ok(rows) = words_conn(app).and_then(|conn| words::recent(&conn, limit)) else { return Vec::new() };
    rows.into_iter().map(|(word, pos)| Recent { word, pos }).collect()
}
