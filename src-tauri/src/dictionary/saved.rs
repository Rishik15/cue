//! Purpose: The saved-words database connection and the commands of the Saved Words page.
//! Contents: words_conn — open (and create) `words.db` in the data folder; list_words / delete_word — commands for the page.

use tauri::{AppHandle, Manager};

use super::words;

pub(super) fn words_conn(app: &AppHandle) -> Result<rusqlite::Connection, String> {
    let dir = app.path().app_data_dir().map_err(|e| format!("No place to keep saved words: {e}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("Could not create the data folder: {e}"))?;
    words::open(&dir.join("words.db"))
}

#[tauri::command]
pub async fn list_words(app: AppHandle) -> Result<Vec<words::Saved>, String> {
    crate::snippets::blocking(move || words::list(&words_conn(&app)?)).await
}

#[tauri::command]
pub async fn delete_word(app: AppHandle, id: i64) -> Result<(), String> {
    crate::snippets::blocking(move || words::remove(&words_conn(&app)?, id)).await
}
