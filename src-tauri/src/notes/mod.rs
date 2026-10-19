//! Purpose: Quick notes: a shortcut opens a small always-on-top note window that takes the keyboard at once and saves as you type. The window is built when pressed and destroyed when closed, so it costs nothing at rest.
//! Contents: on_press — the shortcut: open, focus or close; note — the page's one command (`Cmd` in, `Reply` out) over `store`; go / arrive — moving between notes.
//! `window` builds and places the window; the first note travels inside the page (`start`), so the editor paints with its text.

mod eval;
mod link;
mod math;
mod stats;
mod store;
mod timer;
mod window;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Instant;

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
    theme: String,
    checked: String,
    digits: u64,
    thousands: bool,
}

fn options(app: &AppHandle) -> Options {
    Options {
        text_size: settings::get_u64(app, "notes_text_size", 14),
        paper: settings::get_str(app, "notes_paper").unwrap_or_else(|| "Plain".into()),
        theme: settings::get_str(app, "notes_theme").unwrap_or_else(|| "Match Cue".into()),
        checked: settings::get_str(app, "notes_checked").unwrap_or_else(|| "Stay".into()),
        digits: settings::get_u64(app, "notes_digits", 2),
        thousands: settings::get_bool(app, "notes_thousands", true),
    }
}

/// The window is being built: a second press meanwhile would build a second one.
static OPENING: AtomicBool = AtomicBool::new(false);
/// When the press that is opening the window came, for the timing line in the log.
static PRESSED: Mutex<Option<Instant>> = Mutex::new(None);

/// A line in `cue.log`: how long since the press, so a slow open can be told from a slow page.
fn lap(what: &str) {
    let since = PRESSED.lock().unwrap_or_else(|e| e.into_inner()).map(|t| t.elapsed().as_millis());
    crate::log::note(&format!("notes: {what} after {} ms", since.unwrap_or(0)));
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
    if crate::busy::busy(app) || OPENING.swap(true, Ordering::AcqRel) {
        return; // dictating, or the window is still being built from an earlier press
    }
    *PRESSED.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now());
    let handle = app.clone();
    // The webview is the slow part: built on a worker thread, never the hook's.
    if let Err(e) = guard::spawn("notes-open", 512 * 1024, move || {
        if let Err(e) = start(&handle) {
            status::report(&handle, &format!("Could not open the note: {e}"));
        }
        OPENING.store(false, Ordering::Release);
    }) {
        OPENING.store(false, Ordering::Release);
        status::report(app, &format!("Could not open the note: {e}"));
    }
}

fn start(app: &AppHandle) -> Result<(), String> {
    let open = with_db(app, |c| {
        store::sweep(c, settings::get_i64(app, "notes_expire_days", 0))?;
        open_note(c, store::newest(c)?)
    })?;
    lap("note read");
    let start = Start { open, options: options(app), theme: settings::get_theme(app.clone()) };
    let script = format!("window.__NOTE__ = {};", serde_json::to_string(&start).map_err(|e| e.to_string())?);
    let built = window::build(app, script);
    lap("window built");
    built
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
    /// Open a link from the note in the browser.
    Link {
        url: String,
    },
    /// Start (`on`) or stop passing copied text to the note window (a `paste` note collecting what is copied).
    Collect {
        on: bool,
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
    Evaluated(eval::Evaluated),
    Timer(Option<timer::View>),
    Done,
}

/// Text copied anywhere goes to the page as the `notes-copied` event while collecting (the page adds it to the note, so undo and saving work as for typing); it stops when asked and when the window closes.
fn collect(app: &AppHandle, on: bool) -> Result<(), String> {
    if !on {
        crate::platform::stop_clipboard_watch();
        return Ok(());
    }
    let handle = app.clone();
    crate::platform::watch_clipboard(Box::new(move |text| {
        let _ = handle.emit_to(window::WINDOW, "notes-copied", text);
    }))
}

#[tauri::command]
pub async fn note(app: AppHandle, cmd: Cmd) -> Result<Reply, String> {
    match cmd {
        Cmd::Ready => {
            lap("painted");
            Ok(Reply::Done)
        }
        Cmd::Timer { id, spec, start } => {
            let line = spec.map(|s| s.chars().take(200).collect::<String>());
            Ok(Reply::Timer(timer::set(&app, id, line.as_deref(), start)))
        }
        Cmd::Link { url } => link::open(&url).map(|()| Reply::Done),
        Cmd::Collect { on } => collect(&app, on).map(|()| Reply::Done),
        Cmd::Eval { text, selection, digits, thousands } => eval::evaluate(&text, selection.as_deref(), digits, thousands),
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
        Cmd::Ready | Cmd::Close { .. } | Cmd::Eval { .. } | Cmd::Timer { .. } | Cmd::Collect { .. } | Cmd::Link { .. } => Ok(Reply::Done),
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
