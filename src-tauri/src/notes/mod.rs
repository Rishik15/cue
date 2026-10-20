//! Purpose: Quick notes: a shortcut opens a small always-on-top note window that takes the keyboard at once and saves as you type. The window is built when pressed and destroyed when closed, so it costs nothing at rest.
//! Contents: on_press — the shortcut: open, focus or close; note — the page's one command (`Cmd` in, `Reply` out) over `store`; go / arrive — moving between notes.
//! `image` reads a dropped picture. `window` builds and places the window; the first note travels inside the page (`start`), so the editor paints with its text.

pub mod breaks;
mod eval;
mod image;
mod link;
mod math;
pub(crate) mod start;
mod store;
mod timer;
mod welcome;
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

/// The note on screen.
#[derive(Serialize)]
pub struct Open {
    note: Note,
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
            // Bringing a window to the front waits for Windows (up to half a second): not on the hotkey's thread.
            let handle = app.clone();
            let _ = guard::spawn("notes-show", 128 * 1024, move || window::show(&handle));
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
        if let Err(e) = open_window(&handle) {
            status::report(&handle, &format!("Could not open the note: {e}"));
        }
        OPENING.store(false, Ordering::Release);
    }) {
        OPENING.store(false, Ordering::Release);
        status::report(app, &format!("Could not open the note: {e}"));
    }
}

fn open_window(app: &AppHandle) -> Result<(), String> {
    let first = welcome::pending(app);
    let (first, welcome) = (first.as_deref() == Some("first"), first.filter(|w| w != "first")); // a new install gets the intro as a note, not a page over one
    let open = with_db(app, |c| {
        store::sweep(c, settings::get_i64(app, "notes_expire_days", 0))?;
        let mut note = store::newest(c)?;
        if first {
            welcome::seed(c, &mut note)?;
        }
        Ok(Open { note })
    })?;
    lap("note read");
    let options = start::options(app);
    let answers = eval::answers(&open.note.body, options.digits.min(7) as u8, options.thousands);
    let start = start::Start { open, options, answers, theme: settings::get_theme(app.clone()), welcome };
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
    /// A note's text to be worked out: the math result of each line.
    Eval {
        text: String,
        digits: u8,
        thousands: bool,
    },
    /// The timer of a note: `line` is a line of the note the cursor just left (None only reports the note's timer).
    Timer {
        id: i64,
        line: Option<String>,
    },
    /// Pause (toggles), restart or stop the timer of a note.
    TimerCtl {
        id: i64,
        ctl: timer::Control,
    },
    /// Open a link from the note in the browser.
    Link {
        url: String,
    },
    /// Start (`on`) or stop passing copied text to the note window (a `paste` note collecting what is copied).
    Collect {
        on: bool,
    },
    /// A picture dropped on the window: its text.
    Image {
        path: String,
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
    Text(String),
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
            // Bringing a window to the front waits for Windows (up to half a second): not on the async pool's thread.
            blocking(move || {
                window::show(&app);
                window::focus(&app);
                Ok(Reply::Done)
            })
            .await
        }
        Cmd::Timer { id, line } => {
            let line = line.map(|s| s.chars().take(200).collect::<String>());
            Ok(Reply::Timer(timer::set(&app, id, line.as_deref())))
        }
        Cmd::TimerCtl { id, ctl } => Ok(Reply::Timer(timer::control(&app, id, ctl))),
        Cmd::Link { url } => link::open(&url).map(|()| Reply::Done),
        Cmd::Image { path } => blocking(move || image::read(&path).map(Reply::Text)).await,
        Cmd::Collect { on } => collect(&app, on).map(|()| Reply::Done),
        Cmd::Eval { text, digits, thousands } => eval::evaluate(&text, digits, thousands),
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
        Cmd::Trash { id } => store::trash(c, id).and_then(|()| store::newest(c)).map(|note| Reply::Open(Open { note })),
        Cmd::Restore { id } => store::restore(c, id).and_then(|()| shown(c, id)),
        Cmd::Promote { id } => store::promote(c, id).and_then(|()| shown(c, id)),
        Cmd::Pin { id, pinned } => store::pin(c, id, pinned).map(|()| Reply::Done),
        Cmd::List { query, trash } => store::list(c, &query, trash).map(Reply::Rows),
        Cmd::Ready
        | Cmd::Close { .. }
        | Cmd::Eval { .. }
        | Cmd::Timer { .. }
        | Cmd::TimerCtl { .. }
        | Cmd::Collect { .. }
        | Cmd::Link { .. }
        | Cmd::Image { .. } => Ok(Reply::Done),
    }
}

fn shown(c: &Connection, id: i64) -> Result<Reply, String> {
    Ok(Reply::Open(Open { note: store::get(c, id)?.ok_or("The note is gone.")? }))
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
    let Some(target) = target.filter(|t| t.id != current.id) else { return Ok(Open { note: current }) };
    store::forget_empty(c, current.id)?;
    Ok(Open { note: target })
}
