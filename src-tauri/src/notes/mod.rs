//! Purpose: Quick notes: a shortcut opens a small always-on-top note window that takes the keyboard at once and saves as you type. The window is built when pressed and destroyed when closed, so it costs nothing at rest.
//! Contents: on_press — the shortcut: open, focus or close; note — the page's one command (`Cmd` in, `Reply` out) over `store`; go / arrive — moving between notes.
//! `window` builds and places the window; the first note travels inside the page (`start`), so the editor paints with its text.

mod math;
mod stats;
mod store;
mod timer;
mod window;

use std::path::PathBuf;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
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

/// How to move from the note on screen.
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Move {
    Older,
    Newer,
    Newest,
    New,
}

/// Everything the page can ask for, as one command: the page has one door and Rust one place that validates it.
#[derive(Deserialize)]
#[serde(tag = "do", rename_all = "lowercase")]
pub enum Cmd {
    Save {
        id: i64,
        body: String,
    },
    Go {
        from: i64,
        to: Move,
    },
    Open {
        from: i64,
        id: i64,
    },
    Trash {
        id: i64,
    },
    Restore {
        id: i64,
    },
    Pin {
        id: i64,
        pinned: bool,
    },
    Promote {
        id: i64,
    },
    List {
        query: String,
        trash: bool,
    },
    /// A note's text (and the selection, if any) to be worked out: math results per line and the counts for the status line.
    Eval {
        text: String,
        selection: Option<String>,
        digits: u8,
        thousands: bool,
    },
    /// The timer of a note: `spec` is the note's first line (None stops it); a timer starts only with `start` (the cursor has left that line).
    Timer {
        id: i64,
        spec: Option<String>,
        start: bool,
    },
    /// The page has painted: show the window and give it the keyboard.
    Ready,
    /// Close the window; the note on screen is dropped if it is empty.
    Close {
        id: i64,
    },
}

/// The answer: the note to show, the rows of the list, or nothing.
#[derive(Serialize)]
#[serde(untagged)]
pub enum Reply {
    Open(Open),
    Rows(Vec<Entry>),
    Evaluated(Evaluated),
    Timer(Option<timer::View>),
    Done,
}

/// Math results by line (None where a line shows nothing) and the counts the status line shows.
#[derive(Serialize)]
pub struct Evaluated {
    results: Vec<Option<String>>,
    stats: Option<stats::Stats>,
}

/// Needs no database, so it runs on the spot.
fn evaluate(text: &str, selection: Option<&str>, digits: u8, thousands: bool) -> Result<Reply, String> {
    if text.len() > store::MAX_BODY * 4 {
        return Err("The note is too long to work out.".into());
    }
    let look = math::Look { digits: usize::from(digits.min(7)), thousands };
    let first = text.split('\n').next().unwrap_or("");
    let stats = match selection.filter(|s| !s.trim().is_empty()) {
        Some(selection) => Some(stats::of(selection, &look)),
        None if stats::keyword(first) => Some(stats::of(text.split_once('\n').map_or("", |(_, rest)| rest), &look)),
        None => None,
    };
    Ok(Reply::Evaluated(Evaluated { results: math::evaluate(text, &look), stats }))
}

#[tauri::command]
pub async fn note(app: AppHandle, cmd: Cmd) -> Result<Reply, String> {
    match cmd {
        Cmd::Ready => {
            window::show(&app);
            Ok(Reply::Done)
        }
        Cmd::Timer { id, spec, start } => {
            let line = spec.map(|s| s.chars().take(200).collect::<String>());
            Ok(Reply::Timer(timer::set(&app, id, line.as_deref(), start)))
        }
        Cmd::Eval { text, selection, digits, thousands } => evaluate(&text, selection.as_deref(), digits, thousands),
        Cmd::Close { id } => {
            window::close(&app);
            blocking(move || with_db(&app, |c| store::forget_empty(c, id).map(|()| Reply::Done))).await
        }
        cmd => blocking(move || with_db(&app, |c| run(c, cmd))).await,
    }
}

fn run(c: &Connection, cmd: Cmd) -> Result<Reply, String> {
    match cmd {
        Cmd::Save { id, body } => store::save(c, id, &body).map(|()| Reply::Done),
        Cmd::Go { from, to } => go(c, from, to).map(Reply::Open),
        Cmd::Open { from, id } => {
            let current = store::get(c, from)?.ok_or("The note is gone.")?;
            arrive(c, current, store::get(c, id)?).map(Reply::Open)
        }
        // The newest note is shown afterwards: right when the note on screen was trashed, ignored by the list otherwise.
        Cmd::Trash { id } => store::trash(c, id).and_then(|()| open_note(c, store::newest(c)?)).map(Reply::Open),
        Cmd::Restore { id } => store::restore(c, id).and_then(|()| shown(c, id)),
        Cmd::Promote { id } => store::promote(c, id).and_then(|()| shown(c, id)),
        Cmd::Pin { id, pinned } => store::pin(c, id, pinned).map(|()| Reply::Done),
        Cmd::List { query, trash } => store::list(c, &query, trash).map(Reply::Rows),
        Cmd::Ready | Cmd::Close { .. } | Cmd::Eval { .. } | Cmd::Timer { .. } => Ok(Reply::Done),
    }
}

fn shown(c: &Connection, id: i64) -> Result<Reply, String> {
    open_note(c, store::get(c, id)?.ok_or("The note is gone.")?).map(Reply::Open)
}

/// Moves from the note on screen. Past the newest it starts a new note; an empty note that is left is dropped, and an empty note never starts another.
fn go(c: &Connection, from: i64, to: Move) -> Result<Open, String> {
    let current = store::get(c, from)?.ok_or("The note is gone.")?;
    let target = match to {
        Move::Older => store::neighbour(c, from, false)?,
        Move::Newer => match store::neighbour(c, from, true)? {
            Some(note) => Some(note),
            None => fresh(c, &current)?,
        },
        Move::Newest => Some(store::newest(c)?),
        Move::New => fresh(c, &current)?,
    };
    arrive(c, current, target)
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
