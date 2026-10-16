//! Purpose: Saved snippets: the text that replaces a trigger typed anywhere (see `expand`), the commands the Snippets page uses, and the shortcut that opens them as a palette to insert from.
//! Contents: on_press — open the snippets palette; record_use — count an inserted snippet; list_snippets (snippets and folders) / save_snippet / delete_snippet / save_folder / delete_folder / import_snippets / export_snippets / snippet_preview / read_clipboard_text — commands for the page.
//! Storage is `store` (SQLite, opened per operation) over `model`, the file format is `transfer`, typing-time expansion is `expand`. Every change re-reads the expander's triggers.

pub mod ask;
mod check;
mod expand;
mod folders;
mod foreign;
mod model;
#[cfg(test)]
mod props;
mod store;
mod transfer;

use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;

pub use expand::sync as sync_expansion;

use crate::palette::{Group, Item};
use crate::platform;
use crate::preview;
use crate::status;
use model::{Draft, Folder, FolderDraft, Snippet};

fn db_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| format!("No place to keep snippets: {e}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("Could not create the data folder: {e}"))?;
    Ok(dir.join("snippets.db"))
}

/// Commands run their database work on a worker thread: SQLite waits on locks and the disk, and a plain command would run on the main thread.
pub(crate) async fn blocking<T: Send + 'static>(work: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(work).await.map_err(|e| e.to_string())?
}

fn with_db<T>(app: &AppHandle, f: impl FnOnce(&rusqlite::Connection) -> Result<T, String>) -> Result<T, String> {
    f(&store::open(&db_path(app)?)?)
}

/// The shortcut: open the snippets palette (search by name or trigger, see what each one expands to) to insert into the app that has the focus.
pub fn on_press(app: &AppHandle) {
    if crate::dictation::is_recording() || preview::is_open(app) {
        return;
    }
    let target = platform::foreground_window();
    let app = app.clone();
    // Own thread: the hotkey thread must not wait for the database or the window.
    let spawned = crate::guard::spawn("snippets", 512 * 1024, move || {
        let rows = match with_db(&app, store::list) {
            Ok(rows) => rows,
            Err(e) => return status::report(&app, &e),
        };
        let items = rows.iter().filter(|s| s.enabled).map(|s| item(&app, s)).collect();
        preview::open_snippets(&app, items, platform::foreground_app(), target);
    });
    if let Err(e) = spawned {
        eprintln!("cue: could not start the snippets flow: {e}");
    }
}

/// A row of the palette: the name, the trigger beside it, and the text it expands to (placeholders filled in) on the right.
fn item(app: &AppHandle, s: &Snippet) -> Item {
    Item {
        id: format!("snippet:{}", s.id),
        name: s.name.clone(),
        hint: s.triggers.first().cloned().unwrap_or_default(),
        group: Group::Snippet,
        model: false,
        result: Some(shown(s, expand::preview(app, Some(s.id), &s.body))),
    }
}

/// What a row shows and a palette insert pastes: a formatted snippet as plain text, since the palette pastes text only.
fn shown(s: &Snippet, text: String) -> String {
    if s.insert == model::Insert::Rich {
        expand::richtext::to_plain(&text)
    } else {
        text
    }
}

/// A snippet was inserted from the palette: count it, so the ones used most come first.
pub fn record_use(app: &AppHandle, id: i64) {
    let _ = with_db(app, |c| store::bump(c, id));
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

#[tauri::command]
pub async fn list_snippets(app: AppHandle) -> Result<Library, String> {
    blocking(move || with_db(&app, |c| Ok(Library { snippets: store::list(c)?, folders: folders::folders(c)? }))).await
}

/// Writes a change, then brings the expander in line with it.
async fn change(app: AppHandle, work: impl FnOnce(&rusqlite::Connection) -> Result<(), String> + Send + 'static) -> Result<(), String> {
    blocking(move || {
        with_db(&app, work)?;
        sync_expansion(&app);
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn save_snippet(app: AppHandle, id: Option<i64>, draft: Draft) -> Result<(), String> {
    change(app, move |c| store::save(c, id, &draft)).await
}

#[tauri::command]
pub async fn delete_snippet(app: AppHandle, id: i64) -> Result<(), String> {
    change(app, move |c| store::remove(c, id)).await
}

/// Adds a folder (`id` None) or changes one; returns its id.
#[tauri::command]
pub async fn save_folder(app: AppHandle, id: Option<i64>, draft: FolderDraft) -> Result<i64, String> {
    blocking(move || {
        let saved = with_db(&app, |c| folders::save_folder(c, id, &draft))?;
        sync_expansion(&app);
        Ok(saved)
    })
    .await
}

/// Deletes a folder; its snippets stay, unfiled.
#[tauri::command]
pub async fn delete_folder(app: AppHandle, id: i64) -> Result<(), String> {
    change(app, move |c| folders::remove_folder(c, id)).await
}

/// The text on the clipboard, to start a snippet from it.
#[tauri::command]
pub async fn read_clipboard_text() -> Option<String> {
    tauri::async_runtime::spawn_blocking(platform::clipboard_text).await.ok().flatten()
}

/// What a snippet's text comes to now, with its placeholders filled in (the editor's preview).
#[tauri::command]
pub async fn snippet_preview(app: AppHandle, id: Option<i64>, body: String) -> Result<String, String> {
    blocking(move || Ok(expand::preview(&app, id, &body))).await
}

#[derive(Serialize)]
pub struct Imported {
    added: usize,
    skipped: usize,
}

/// Stores parsed entries (duplicates and clashing triggers are skipped) and brings the expander up to date.
fn add_parsed(app: &AppHandle, entries: &[(Draft, String)], refused: usize) -> Result<Imported, String> {
    let added = with_db(app, |c| store::add_all(c, entries))?;
    sync_expansion(app);
    Ok(Imported { added, skipped: entries.len() - added + refused })
}

/// A few snippets to start from (dates, an id, a signoff); their triggers start with `;`.
#[tauri::command]
pub async fn add_starter_snippets(app: AppHandle) -> Result<Imported, String> {
    blocking(move || {
        let (entries, refused) = transfer::parse(include_str!("starter.json"))?;
        add_parsed(&app, &entries, refused)
    })
    .await
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
        let size = std::fs::metadata(&path).map_err(|e| format!("Could not read the file: {e}"))?.len();
        if size > transfer::MAX_FILE_BYTES {
            return Err("This file is too large to be a snippets file.".into());
        }
        let text = std::fs::read_to_string(&path).map_err(|e| format!("Could not read the file: {e}"))?;
        let extension = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        let (entries, refused) = foreign::parse(&extension, &text)?;
        add_parsed(&app, &entries, refused).map(Some)
    })
    .await
}

#[tauri::command]
pub async fn export_snippets(app: AppHandle) -> Result<Option<usize>, String> {
    blocking(move || {
        let Some(picked) =
            app.dialog().file().add_filter("Cue snippets", &["json"]).set_file_name("cue-snippets.json").blocking_save_file()
        else {
            return Ok(None);
        };
        let path = picked.into_path().map_err(|e| format!("Could not use that file: {e}"))?.to_string_lossy().into_owned();
        let (all, groups) = with_db(&app, |c| Ok((store::list(c)?, folders::folders(c)?)))?;
        // Written beside the target and renamed over it, so a failure half-way never leaves a broken file where a good one was.
        let temp = format!("{path}.cue-tmp");
        std::fs::write(&temp, transfer::export(&all, &groups)).and_then(|()| std::fs::rename(&temp, &path)).map_err(|e| {
            let _ = std::fs::remove_file(&temp);
            format!("Could not write the file: {e}")
        })?;
        Ok(Some(all.len()))
    })
    .await
}
