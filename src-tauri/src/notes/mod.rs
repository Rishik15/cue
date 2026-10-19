//! Purpose: Quick notes: a shortcut opens a small always-on-top note window that takes the keyboard at once and saves as you type. The window is built when pressed and destroyed when closed, so it costs nothing at rest.
//! Contents: on_press — the shortcut: open, focus or close; note_save / note_go / note_open / note_trash / note_restore / note_pin / note_promote / note_list — the page's commands over `store`;
//! note_ready / note_close — show the painted window, close it. `window` builds and places it; the first note travels inside the page (`start`), so the editor paints with its text.

mod store;
mod window;

use std::path::PathBuf;

use rusqlite::Connection;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::snippets::blocking;
use crate::{guard, settings, status};
use store::{Entry, Note};

fn db_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| format!("No place to keep notes: {e}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("Could not create the data folder: {e}"))?;
    Ok(dir.join("notes.db"))
}

fn with_db<T>(app: &AppHandle, f: impl FnOnce(&Connection) -> Result<T, String>) -> Result<T, String> {
    f(&store::open(&db_path(app)?)?)
}

/// A note on screen: its place in the order (1 is the newest) and how many there are.
#[derive(Serialize)]
pub struct Open {
    note: Note,
    index: i64,
    count: i64,
}

fn open_note(conn: &Connection, note: Note) -> Result<Open, String> {
    let (index, count) = store::position(conn, note.id)?;
    Ok(Open { note, index, count })
}

/// What the page needs before it paints: the note, how it looks and the theme.
#[derive(Serialize)]
struct Start {
    open: Open,
    options: Options,
    theme: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Serialize)]
struct Options {
    text_size: u64,
    paper: String,
    checked: String,
    digits: u64,
    thousands: bool,
}

fn options(app: &AppHandle) -> Options {
    Options {
        text_size: settings::get_u64(app, "notes_text_size", 14),
        paper: settings::get_str(app, "notes_paper").unwrap_or_else(|| "Plain".into()),
        checked: settings::get_str(app, "notes_checked").unwrap_or_else(|| "Stay".into()),
        digits: settings::get_u64(app, "notes_digits", 2),
        thousands: settings::get_bool(app, "notes_thousands", true),
    }
}

/// The shortcut: closed opens the newest note, open but behind comes to the front, in front closes (the page saves first).
pub fn on_press(app: &AppHandle) {
    if window::exists(app) {
        if window::is_focused(app) {
            let _ = app.emit_to(window::WINDOW, "notes-close", ());
        } else {
            window::show(app);
        }
        return;
    }
    if crate::busy::busy(app) {
        return;
    }
    let handle = app.clone();
    // The webview is the slow part: built on a worker thread, never the hook's.
    if let Err(e) = guard::spawn("notes-open", 512 * 1024, move || {
        if let Err(e) = start(&handle) {
            status::report(&handle, &format!("Could not open the note: {e}"));
        }
    }) {
        status::report(app, &format!("Could not open the note: {e}"));
    }
}

fn start(app: &AppHandle) -> Result<(), String> {
    let open = with_db(app, |c| {
        store::sweep(c, settings::get_i64(app, "notes_expire_days", 0))?;
        open_note(c, store::newest(c)?)
    })?;
    let start = Start { open, options: options(app), theme: settings::get_theme(app.clone()) };
    let script = format!("window.__NOTE__ = {};", serde_json::to_string(&start).map_err(|e| e.to_string())?);
    window::build(app, script)
}

#[tauri::command]
pub async fn note_save(app: AppHandle, id: i64, body: String) -> Result<(), String> {
    blocking(move || with_db(&app, |c| store::save(c, id, &body))).await
}

/// Moves from the note on screen: `older`, `newer` (past the newest it starts a new note), `newest` or `new`. An empty note that is left is dropped, and an empty note never starts another.
#[tauri::command]
pub async fn note_go(app: AppHandle, from: i64, to: String) -> Result<Open, String> {
    blocking(move || {
        with_db(&app, |c| {
            let current = store::get(c, from)?.ok_or("The note is gone.")?;
            let target = match to.as_str() {
                "older" => store::neighbour(c, from, false)?,
                "newer" => match store::neighbour(c, from, true)? {
                    Some(note) => Some(note),
                    None => fresh(c, &current)?,
                },
                "newest" => Some(store::newest(c)?),
                "new" => fresh(c, &current)?,
                _ => return Err(format!("Unknown move: {to}")),
            };
            arrive(c, current, target)
        })
    })
    .await
}

/// A new note, unless the one on screen is empty already.
fn fresh(c: &Connection, current: &Note) -> Result<Option<Note>, String> {
    if current.body.is_empty() {
        return Ok(None);
    }
    store::create(c).map(Some)
}

/// Leaves `current` (dropping it if empty) for `target`; no target means stay.
fn arrive(c: &Connection, current: Note, target: Option<Note>) -> Result<Open, String> {
    let Some(target) = target.filter(|t| t.id != current.id) else { return open_note(c, current) };
    store::forget_empty(c, current.id)?;
    open_note(c, target)
}

/// Opens a note picked from the list.
#[tauri::command]
pub async fn note_open(app: AppHandle, from: i64, id: i64) -> Result<Open, String> {
    blocking(move || {
        with_db(&app, |c| {
            let current = store::get(c, from)?.ok_or("The note is gone.")?;
            arrive(c, current, store::get(c, id)?)
        })
    })
    .await
}

/// Moves the note to the trash and shows the next one (the newest, or a fresh note when none is left).
#[tauri::command]
pub async fn note_trash(app: AppHandle, id: i64) -> Result<Open, String> {
    blocking(move || {
        with_db(&app, |c| {
            store::trash(c, id)?;
            open_note(c, store::newest(c)?)
        })
    })
    .await
}

/// Brings a note back from the trash and shows it.
#[tauri::command]
pub async fn note_restore(app: AppHandle, id: i64) -> Result<Open, String> {
    blocking(move || {
        with_db(&app, |c| {
            store::restore(c, id)?;
            open_note(c, store::get(c, id)?.ok_or("The note is gone.")?)
        })
    })
    .await
}

#[tauri::command]
pub async fn note_pin(app: AppHandle, id: i64, pinned: bool) -> Result<(), String> {
    blocking(move || with_db(&app, |c| store::pin(c, id, pinned))).await
}

/// Makes the note the newest in the order and returns its new place.
#[tauri::command]
pub async fn note_promote(app: AppHandle, id: i64) -> Result<Open, String> {
    blocking(move || {
        with_db(&app, |c| {
            store::promote(c, id)?;
            open_note(c, store::get(c, id)?.ok_or("The note is gone.")?)
        })
    })
    .await
}

#[tauri::command]
pub async fn note_list(app: AppHandle, query: String, trash: bool) -> Result<Vec<Entry>, String> {
    blocking(move || with_db(&app, |c| store::list(c, &query, trash))).await
}

/// The page has painted: show the window and give it the keyboard.
#[tauri::command]
pub fn note_ready(app: AppHandle) {
    window::show(&app);
}

/// Closes the window; the note on screen is dropped if it is empty.
#[tauri::command]
pub async fn note_close(app: AppHandle, id: i64) -> Result<(), String> {
    window::close(&app);
    blocking(move || with_db(&app, |c| store::forget_empty(c, id))).await
}
