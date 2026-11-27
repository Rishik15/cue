//! File: snippets/mod.rs
//! Purpose: Saved snippets: the text that replaces a trigger typed anywhere (see `expand`), the commands the Snippets page uses.
//! Contents: list_snippets (snippets and folders) / save_snippet / delete_snippet / save_folder / delete_folder / import_snippets / export_snippets / snippet_preview / read_clipboard_text — commands for the page; read_library — the snippets and folders as one committed state (list and export).
//! Storage is `store` (SQLite, opened per operation; `bulk` for imports) over `model`, the page's rules are `policy` (with their settings in `keys`), the file format is `transfer`, typing-time expansion is `expand`. Every change re-reads the expander's triggers (`persist`); an import file is read once from one handle within `transfer::MAX_FILE_BYTES`. Nothing outside this module reaches into its siblings: the commands, `sync_expansion`, `check_setting`, `change_setting`, `ask` and `blocking` are its face.

pub mod ask;
mod bulk;
mod check;
mod expand;
mod folders;
mod foreign;
mod keys;
mod model;
mod policy;
mod store;
mod transfer;

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;

pub use expand::sync as sync_expansion;
pub use keys::check as check_setting;

use crate::lock::lock;
use crate::platform;
use model::{Draft, Folder, FolderDraft, Snippet};

/// Where the snippets file lives, the data folder created when missing. Input: the app. Returns the path.
fn db_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| format!("No place to keep snippets: {e}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("Could not create the data folder: {e}"))?;
    Ok(dir.join("snippets.db"))
}

/// Commands run their database work on a worker thread: SQLite waits on locks and the disk, and a plain command would run on the main thread.
pub(crate) async fn blocking<T: Send + 'static>(work: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(work).await.map_err(|e| e.to_string())?
}

/// Held around every snippet write and every setting change, so a check and the write it allows are one step against the other side (the settings file is not in SQLite, so one transaction cannot hold both).
static WRITES: Mutex<()> = Mutex::new(());

/// Saves a setting with `save`, unless it is the timing or match case and the change would make two stored triggers clash; then it is refused with the pair named. Other keys are saved at once, never behind a snippet write. Inputs: the app, the key, the new value and the write. Returns what `save` returns, or the refusal. Blocking; call it from a worker thread.
pub fn change_setting(
    app: &AppHandle,
    key: &str,
    value: &serde_json::Value,
    save: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    if key != keys::MODE && key != keys::MATCH_CASE {
        return save();
    }
    checked_setting(&db_path(app)?, policy::load, key, value, save)
}

/// `change_setting` on the snippets file at `db`, with `rules` read once it is this change's turn. Inputs: the file, the current rules, the key, the value and the write. Returns what `save` returns, or the refusal.
fn checked_setting(
    db: &Path,
    rules: impl FnOnce() -> policy::Policy,
    key: &str,
    value: &serde_json::Value,
    save: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    let _writing = lock(&WRITES);
    if let Some(next) = rules().changed(key, value) {
        store::check_library(&store::open(db)?, &next)?;
    }
    save()
}

/// Runs a write on the snippets file at `db`, opened for this call only. Inputs: the file and the work. Returns what `work` returns.
fn write_db<T>(db: &Path, work: impl FnOnce(&rusqlite::Connection) -> Result<T, String>) -> Result<T, String> {
    let _writing = lock(&WRITES);
    work(&store::open(db)?)
}

/// Runs `f` on the snippets file, opened for this call only. Inputs: the app and the work. Returns what `f` returns; the error type is the caller's (`store::Failure` keeps SQLite's error typed, `String` is the page's message).
fn with_db<T, E: From<store::Failure> + From<String>>(
    app: &AppHandle,
    f: impl FnOnce(&rusqlite::Connection) -> Result<T, E>,
) -> Result<T, E> {
    f(&store::open(&db_path(app)?)?)
}

/// The snippets and folders as one committed state of the file (one read transaction). Input: the connection. Returns them.
fn read_library(c: &rusqlite::Connection) -> Result<Library, store::Failure> {
    store::snapshot(c, |c| Ok(Library { snippets: store::list(c)?, folders: folders::folders(c)? }))
}

/// Everything the Snippets page shows.
#[derive(Serialize)]
pub struct Library {
    snippets: Vec<Snippet>,
    folders: Vec<Folder>,
}

/// Every program a person could name (installed, and running ones the Start menu does not list), for picking a program from a list instead of typing its name.
#[tauri::command]
pub async fn list_apps() -> Result<Vec<String>, String> {
    blocking(|| Ok(platform::list_apps())).await
}

/// The snippets and folders for the page.
#[tauri::command]
pub async fn list_snippets(app: AppHandle) -> Result<Library, String> {
    blocking(move || with_db(&app, read_library).map_err(String::from)).await
}

/// Writes a change, then brings the expander in line with it: the one place that orders the two. Blocking; call it from a worker thread.
fn persist<T>(app: &AppHandle, work: impl FnOnce(&rusqlite::Connection) -> Result<T, String>) -> Result<T, String> {
    let done = write_db(&db_path(app)?, work)?;
    sync_expansion(app);
    Ok(done)
}

/// `persist` on a worker thread, for a command.
async fn change<T: Send + 'static>(
    app: AppHandle,
    work: impl FnOnce(&rusqlite::Connection) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    blocking(move || persist(&app, work)).await
}

/// Adds a snippet (`id` None) or changes one, under the page's rules.
#[tauri::command]
pub async fn save_snippet(app: AppHandle, id: Option<i64>, draft: Draft) -> Result<(), String> {
    change(app, move |c| store::save(c, id, &draft, &policy::load())).await
}

/// Deletes a snippet.
#[tauri::command]
pub async fn delete_snippet(app: AppHandle, id: i64) -> Result<(), String> {
    change(app, move |c| store::remove(c, id)).await
}

/// Adds a folder (`id` None) or changes one; returns its id.
#[tauri::command]
pub async fn save_folder(app: AppHandle, id: Option<i64>, draft: FolderDraft) -> Result<i64, String> {
    change(app, move |c| folders::save_folder(c, id, &draft, &policy::load())).await
}

/// Deletes a folder; its snippets stay, unfiled. Refused when its snippets, switched on by it, would clash.
#[tauri::command]
pub async fn delete_folder(app: AppHandle, id: i64) -> Result<(), String> {
    change(app, move |c| folders::remove_folder(c, id, &policy::load())).await
}

/// The text on the clipboard, to start a snippet from it.
#[tauri::command]
pub async fn read_clipboard_text() -> Option<String> {
    tauri::async_runtime::spawn_blocking(platform::clipboard_text).await.ok().flatten()
}

/// What a snippet's text comes to now, with its placeholders filled in (the editor's preview).
#[tauri::command]
pub async fn snippet_preview(app: AppHandle, id: Option<i64>, body: String) -> Result<String, String> {
    blocking(move || expand::preview(&app, id, &body)).await
}

/// What an import did: snippets added, entries skipped, and includes made literal because they would have read a private setting.
#[derive(Serialize)]
pub struct Imported {
    added: usize,
    skipped: usize,
    escaped: usize,
}

/// Stores parsed entries (duplicates and clashing triggers are skipped) in their folders and brings the expander up to date.
fn add_parsed(app: &AppHandle, file: &transfer::Parsed) -> Result<Imported, String> {
    let (added, escaped) = persist(app, |c| bulk::add_all(c, &file.entries, &file.folders, &policy::load()))?;
    Ok(Imported { added, skipped: file.entries.len() - added + file.refused, escaped })
}

/// A few snippets to start from (dates, an id, a signoff); their triggers start with `;`.
#[tauri::command]
pub async fn add_starter_snippets(app: AppHandle) -> Result<Imported, String> {
    blocking(move || add_parsed(&app, &transfer::parse(include_str!("starter.json"))?)).await
}

/// Asks for a file with the system dialog, opened here so the page never supplies a path. None when the user cancels.
#[tauri::command]
pub async fn import_snippets(app: AppHandle) -> Result<Option<Imported>, String> {
    blocking(move || {
        let Some(picked) = app.dialog().file().add_filter("Snippet files", &["json", "yml", "yaml", "csv", "ahk"]).blocking_pick_file()
        else {
            return Ok(None);
        };
        let path = picked.into_path().map_err(|e| format!("Could not use that file: {e}"))?;
        let file = std::fs::File::open(&path).map_err(|e| format!("Could not read the file: {e}"))?;
        let text = transfer::read_limited(file, transfer::MAX_FILE_BYTES)?;
        let extension = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        add_parsed(&app, &foreign::parse(&extension, &text)?).map(Some)
    })
    .await
}

/// Asks where to save with the system dialog and writes every snippet there in Cue's format. Returns how many were written, None when the user cancels.
#[tauri::command]
pub async fn export_snippets(app: AppHandle) -> Result<Option<usize>, String> {
    blocking(move || {
        let Some(picked) =
            app.dialog().file().add_filter("Cue snippets", &["json"]).set_file_name("cue-snippets.json").blocking_save_file()
        else {
            return Ok(None);
        };
        let path = picked.into_path().map_err(|e| format!("Could not use that file: {e}"))?.to_string_lossy().into_owned();
        let library = with_db(&app, read_library)?;
        crate::fsutil::write_atomic(std::path::Path::new(&path), transfer::export(&library.snippets, &library.folders).as_bytes())
            .map_err(|e| format!("Could not write the file: {e}"))?;
        Ok(Some(library.snippets.len()))
    })
    .await
}

#[cfg(test)]
mod tests;
