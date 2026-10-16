//! Purpose: The dictionary shortcut: look up the selected word in an installed pack and show it in a small popup beside the pointer, with its pronunciation guide and a button that saves it.
//! Contents: on_press — shortcut pressed, run the flow on its own thread (one at a time); run — copy the selection, look it up, open the popup; on_focus / forget — close when the user clicks away, drop the pending word;
//! define_get / define_fit / define_save / define_close — commands for the popup page; list_words / delete_word — commands for the Saved Words page.
//! The popup is a webview created when there is a word and destroyed on close. Packs are opened read-only for one lookup (`pack`), words are saved in `words`.

pub mod catalog;
mod pack;
mod place;
mod words;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, LogicalSize, Manager, PhysicalPosition, WebviewWindowBuilder};

use crate::platform::{self, overlay, Point, Rect};
use crate::prefs::Prefs;
use crate::{preview, status};
use catalog::Pack;
use pack::Block;

const WINDOW: &str = "define";
const WIDTH: f64 = 380.0;
const START_HEIGHT: f64 = 200.0;
/// Space between the pointer and the popup, in logical pixels.
const GAP: f64 = 18.0;

/// A lookup is running (or its popup is open): a second press meanwhile is ignored.
static BUSY: AtomicBool = AtomicBool::new(false);
/// The popup has held the keyboard: clicking away closes it only after that, never while it is still loading.
static HAD_FOCUS: AtomicBool = AtomicBool::new(false);
static PENDING: Mutex<Option<Pending>> = Mutex::new(None);

struct Pending {
    view: View,
    pack: &'static str,
    key: String,
    anchor: Option<(Point, Rect)>,
}

#[derive(Serialize, Clone)]
pub struct View {
    word: String,
    language: String,
    blocks: Vec<Block>,
    saved: bool,
}

pub fn on_press(app: &AppHandle) {
    if crate::dictation::is_recording()
        || preview::is_open(app)
        || app.get_webview_window(WINDOW).is_some()
        || BUSY.swap(true, Ordering::AcqRel)
    {
        return;
    }
    let app = app.clone();
    // Own thread: copying waits for the app and for the shortcut's keys to be released, which must not hold up the hotkey thread.
    let spawned = crate::guard::spawn("define", 512 * 1024, move || {
        let _release = crate::guard::Release(&BUSY);
        run(&app);
    });
    if let Err(e) = spawned {
        BUSY.store(false, Ordering::Release);
        eprintln!("cue: could not start the dictionary flow: {e}");
    }
}

fn notice(app: &AppHandle, text: &str) {
    let prefs = Prefs::load(app);
    overlay::notice(text, prefs.overlay_top, prefs.theme());
}

struct Found {
    pack: &'static Pack,
    language: String,
    blocks: Vec<Block>,
}

/// The first installed pack that knows the word.
fn find_word(app: &AppHandle, key: &str) -> Result<Option<Found>, String> {
    for pack in catalog::installed_packs(app) {
        let Some(path) = catalog::path(app, pack) else { continue };
        let conn = pack::open(&path)?;
        let blocks = pack::define(&conn, key)?;
        if !blocks.is_empty() {
            return Ok(Some(Found { pack, language: pack::language(&conn).unwrap_or_else(|| pack.name.into()), blocks }));
        }
    }
    Ok(None)
}

fn run(app: &AppHandle) {
    let Some((text, _)) = crate::selection::selected_text(app) else { return };
    let Some(key) = pack::normalize(&text) else { return notice(app, "Select one word") };
    if catalog::installed_packs(app).is_empty() {
        return notice(app, "Download a dictionary first");
    }
    let Found { pack, language, blocks } = match find_word(app, &key) {
        Ok(Some(found)) => found,
        Ok(None) => return notice(app, &format!("No definition for {key}")),
        Err(e) => return status::report(app, &e),
    };
    let saved = words_conn(app).is_ok_and(|c| words::is_saved(&c, pack.id, &key));
    let view = View { word: blocks[0].word.clone(), language, blocks, saved };
    *PENDING.lock().unwrap() = Some(Pending { view, pack: pack.id, key, anchor: platform::cursor_anchor() });
    if let Err(e) = open(app) {
        PENDING.lock().unwrap().take();
        status::report(app, &e);
    }
}

fn open(app: &AppHandle) -> Result<(), String> {
    let config = app.config().app.windows.iter().find(|w| w.label == WINDOW).ok_or("The dictionary window is not configured.")?;
    let window = WebviewWindowBuilder::from_config(app, config)
        .and_then(|b| b.build())
        .map_err(|e| format!("Could not open the dictionary: {e}"))?;
    HAD_FOCUS.store(false, Ordering::Release);
    fit(&window, START_HEIGHT);
    Ok(())
}

/// Sets the popup's size and puts it beside the pointer (or in the middle of the screen when the pointer is unknown).
fn fit(window: &tauri::WebviewWindow, height: f64) {
    let scale = window.scale_factor().unwrap_or(1.0);
    let _ = window.set_size(LogicalSize::new(WIDTH, height));
    let anchor = PENDING.lock().unwrap().as_ref().and_then(|p| p.anchor);
    match anchor {
        Some((at, work)) => {
            let size = ((WIDTH * scale) as i32, (height * scale) as i32);
            let (x, y) = place::place(at, work, size, (GAP * scale) as i32);
            let _ = window.set_position(PhysicalPosition::new(x, y));
        }
        None => {
            let _ = window.center();
        }
    }
}

fn close(app: &AppHandle) {
    PENDING.lock().unwrap().take();
    if let Some(window) = app.get_webview_window(WINDOW) {
        let _ = window.destroy();
    }
}

/// Focus moved to or from the popup: it closes when the user clicks away, like the palette.
pub fn on_focus(app: &AppHandle, focused: bool) {
    if focused {
        return HAD_FOCUS.store(true, Ordering::Release);
    }
    if HAD_FOCUS.swap(false, Ordering::AcqRel) {
        close(app);
    }
}

/// The window was destroyed by any means: the word must not linger in memory.
pub fn forget() {
    PENDING.lock().unwrap().take();
}

fn words_conn(app: &AppHandle) -> Result<rusqlite::Connection, String> {
    let dir = app.path().app_data_dir().map_err(|e| format!("No place to keep saved words: {e}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("Could not create the data folder: {e}"))?;
    words::open(&dir.join("words.db"))
}

#[tauri::command]
pub fn define_get() -> Option<View> {
    PENDING.lock().unwrap().as_ref().map(|p| p.view.clone())
}

/// The page measured itself: size and place the window, then show it and give it the keyboard (the app we came from must not stay above it).
#[tauri::command]
pub fn define_fit(app: AppHandle, height: f64) {
    let Some(window) = app.get_webview_window(WINDOW) else { return };
    fit(&window, height.clamp(120.0, 520.0));
    let _ = window.show();
    if let Some(handle) = preview::native_handle(&window) {
        platform::focus_window(handle);
    }
}

/// Saves the word on show (its first meaning); saving it again changes nothing.
#[tauri::command]
pub async fn define_save(app: AppHandle) -> Result<(), String> {
    let (pack, key, view) = {
        let pending = PENDING.lock().unwrap();
        let p = pending.as_ref().ok_or("Nothing to save.")?;
        (p.pack, p.key.clone(), p.view.clone())
    };
    let block = &view.blocks[0];
    let sense = block.senses.first().ok_or("This word has no meaning to save.")?;
    let row = words::Saved {
        id: 0,
        word: block.word.clone(),
        pos: block.pos.clone(),
        ipa: block.ipa.clone().unwrap_or_default(),
        gloss: sense.gloss.clone(),
        example: sense.example.clone().unwrap_or_default(),
        language: view.language,
        added: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64),
    };
    crate::snippets::blocking(move || words::save(&words_conn(&app)?, pack, &key, &row)).await?;
    if let Some(p) = PENDING.lock().unwrap().as_mut() {
        p.view.saved = true;
    }
    Ok(())
}

#[tauri::command]
pub fn define_close(app: AppHandle) {
    close(&app);
}

#[tauri::command]
pub async fn list_words(app: AppHandle) -> Result<Vec<words::Saved>, String> {
    crate::snippets::blocking(move || words::list(&words_conn(&app)?)).await
}

#[tauri::command]
pub async fn delete_word(app: AppHandle, id: i64) -> Result<(), String> {
    crate::snippets::blocking(move || words::remove(&words_conn(&app)?, id)).await
}
