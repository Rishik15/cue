//! File: dictionary/saved.rs
//! Purpose: Looked Up and what uses it: the connection, the commands of the Dictionary page's Looked Up list, recording an Entry the user saw (read first, recorded only once the page shows it) (a Card, or one opened in the Search window) and the Recent words the Search window lists.
//! Contents: words_conn — open (and create) `words.db` in the data folder; list_words / delete_word — commands for the page; record — add an Entry to Looked Up; hold / take_seen — entries read but not yet shown; Recent / recent — the newest words for the Search window.

use crate::lock::lock;
use std::sync::Mutex;

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

/// The Looked Up row of an entry: its first meaning. Input: the entry and the time it was shown (Unix seconds). Return: the row, None when the entry has no meaning.
fn row_of(entry: &Entry, added: i64) -> Option<words::Saved> {
    let block = entry.blocks.first()?;
    let sense = block.senses.first()?;
    Some(words::Saved {
        id: 0,
        word: block.word.clone(),
        pos: block.pos.clone(),
        gloss: sense.gloss.clone(),
        example: sense.example.clone().unwrap_or_default(),
        language: entry.language.clone(),
        added,
    })
}

/// Writes a lookup to the Looked Up list (first meaning only). A failure is only logged: the window must show whatever happens to the list. The Dictionary page reloads on the `words-changed` event this sends instead of on every window focus. Input: the app and the entry that was shown. Return: none.
pub(super) fn record(app: &AppHandle, entry: &Entry) {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
    let Some(row) = row_of(entry, now) else { return };
    if let Err(e) = words_conn(app).and_then(|mut conn| words::record(&mut conn, entry.pack, &entry.key, &row)) {
        crate::log::note(&format!("dictionary: {e}"));
        return;
    }
    let _ = tauri::Emitter::emit(app, "words-changed", ());
}

/// The most entries read but not yet shown that are kept (a user opens one word at a time, so a few cover every overlap).
const HELD: usize = 4;
/// Entries a lookup has read that the page has not yet said it shows, each with the label of the window that read it.
static UNSEEN: Mutex<Vec<(String, Entry)>> = Mutex::new(Vec::new());

/// Keeps `entry` for window `label` until its page says it shows it; the oldest is dropped past `HELD`. Input: the window's label and the entry just read. Return: none.
pub(super) fn hold(label: &str, entry: Entry) {
    hold_in(&mut lock(&UNSEEN), label, entry);
}

/// Takes the entry of `word` window `label` now shows and forgets that window's other ones (the user left them); another window's entries stay. Input: the window's label and the word. Return: the entry, None when that window never held it.
pub(super) fn take_seen(label: &str, word: &str) -> Option<Entry> {
    take_from(&mut lock(&UNSEEN), label, word)
}

/// `hold` on a list: a newer read of the same word by the same window replaces the older. Input: the list, the window's label and the entry. Return: none.
fn hold_in(held: &mut Vec<(String, Entry)>, label: &str, entry: Entry) {
    held.retain(|(owner, e)| owner != label || e.key != entry.key);
    held.push((label.to_owned(), entry));
    if held.len() > HELD {
        held.remove(0);
    }
}

/// `take_seen` on a list. Input: the list, the window's label and the word. Return: the entry for the word.
fn take_from(held: &mut Vec<(String, Entry)>, label: &str, word: &str) -> Option<Entry> {
    let found = held.iter().position(|(owner, e)| owner == label && e.key == word).map(|i| held.remove(i).1);
    held.retain(|(owner, _)| owner != label);
    found
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

#[cfg(test)]
mod tests {
    use super::*;

    /// An entry with only a key.
    fn entry(key: &str) -> Entry {
        Entry { key: key.into(), language: "en".into(), pack: "en", blocks: Vec::new() }
    }

    /// A row takes the first part of speech and first meaning of an entry and the given time; an entry with no meaning gives none.
    #[test]
    fn a_row_is_the_first_meaning_of_an_entry() {
        use super::super::pack::{Block, Sense};
        let sense = |gloss: &str, example: Option<&str>| Sense { gloss: gloss.into(), example: example.map(Into::into) };
        let block = |word: &str, senses| Block { word: word.into(), pos: "noun".into(), senses, synonyms: Vec::new() };
        let mut full = entry("calm");
        full.blocks =
            vec![block("calm", vec![sense("quiet", Some("a calm sea")), sense("still", None)]), block("calm", vec![sense("other", None)])];
        let row = row_of(&full, 42).expect("row");
        assert_eq!(
            (row.word.as_str(), row.gloss.as_str(), row.example.as_str(), row.language.as_str(), row.added),
            ("calm", "quiet", "a calm sea", "en", 42)
        );
        assert!(row_of(&entry("empty"), 1).is_none());
        full.blocks = vec![block("calm", Vec::new())];
        assert!(row_of(&full, 1).is_none());
    }

    /// An entry read but left (Escape, or a newer word) is never taken, and showing one forgets the window's rest.
    #[test]
    fn only_the_shown_entry_is_taken_and_the_windows_rest_are_forgotten() {
        let mut held = Vec::new();
        hold_in(&mut held, "w1", entry("alpha"));
        hold_in(&mut held, "w1", entry("beta"));
        assert_eq!(take_from(&mut held, "w1", "beta").map(|e| e.key), Some("beta".into()));
        assert!(held.is_empty());
        assert!(take_from(&mut held, "w1", "alpha").is_none());
    }

    /// Two windows holding the same word keep their own entry: one's take leaves the other's.
    #[test]
    fn a_window_takes_only_its_own_entry_of_a_word() {
        let mut held = Vec::new();
        hold_in(&mut held, "w1", entry("word"));
        hold_in(&mut held, "w2", entry("word"));
        assert!(take_from(&mut held, "w3", "word").is_none());
        assert!(take_from(&mut held, "w1", "word").is_some());
        assert!(take_from(&mut held, "w2", "word").is_some());
    }

    /// Held entries are capped, oldest first out.
    #[test]
    fn held_entries_are_capped() {
        let mut held = Vec::new();
        for i in 0..HELD + 2 {
            hold_in(&mut held, "w1", entry(&format!("w{i}")));
        }
        assert_eq!(held.len(), HELD);
        assert!(take_from(&mut held.clone(), "w1", "w0").is_none());
    }
}
