//! Purpose: The preview window: shows a text before and after a rule changed it, and applies, copies or drops the result.
//! A webview created when needed and destroyed when closed, so it costs nothing while unused.
//! Contents: show — open (or refresh) the window for a pending result; preview_get / preview_apply / preview_copy / preview_close — commands for the page.

use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WebviewWindowBuilder};

use crate::platform::{self, InsertOptions};
use crate::prefs::Prefs;
use crate::status;

const WINDOW: &str = "preview";

struct Pending {
    original: String,
    result: String,
    /// The window the text came from; Apply brings it back to the front before inserting.
    target: isize,
}

static PENDING: Mutex<Option<Pending>> = Mutex::new(None);

#[derive(Serialize)]
pub struct View {
    original: String,
    result: String,
}

pub fn show(app: &AppHandle, original: String, result: String, target: isize) {
    *PENDING.lock().unwrap() = Some(Pending { original, result, target });
    if let Some(window) = app.get_webview_window(WINDOW) {
        let _ = app.emit_to(WINDOW, "preview-changed", ());
        let _ = window.set_focus();
        return;
    }
    let Some(config) = app.config().app.windows.iter().find(|w| w.label == WINDOW) else { return };
    // Created from the config thread-safely: the caller is a worker thread, never the event loop.
    if let Err(e) = WebviewWindowBuilder::from_config(app, config).and_then(|b| b.build()) {
        status::report(app, &format!("Could not open the preview: {e}"));
    }
}

fn close(app: &AppHandle) {
    PENDING.lock().unwrap().take();
    if let Some(window) = app.get_webview_window(WINDOW) {
        let _ = window.destroy();
    }
}

#[tauri::command]
pub fn preview_get() -> Option<View> {
    PENDING.lock().unwrap().as_ref().map(|p| View { original: p.original.clone(), result: p.result.clone() })
}

#[tauri::command]
pub fn preview_apply(app: AppHandle) {
    let pending = PENDING.lock().unwrap().take();
    close(&app);
    let Some(p) = pending else { return };
    // Own thread: returning focus and typing wait on other programs, and the command must not hold up the window's event loop.
    let handle = app.clone();
    let spawned = std::thread::Builder::new().name("preview-apply".into()).stack_size(512 * 1024).spawn(move || {
        if !platform::focus_window(p.target) {
            return copy(&handle, &p.result, "Cue could not return to the app, so the result was copied to the clipboard.");
        }
        let prefs = Prefs::load(&handle);
        let options = InsertOptions { always_paste: prefs.always_paste, restore_clipboard: prefs.restore_clipboard };
        if let Err(e) = platform::insert(&p.result, &options) {
            status::report(&handle, &e.to_string());
        }
    });
    if let Err(e) = spawned {
        status::report(&app, &format!("Could not apply the result: {e}"));
    }
}

fn copy(app: &AppHandle, text: &str, note: &str) {
    match platform::copy_text(text) {
        Ok(()) if !note.is_empty() => status::report(app, note),
        Ok(()) => {}
        Err(e) => status::report(app, &format!("Could not copy the result: {e}")),
    }
}

#[tauri::command]
pub fn preview_copy(app: AppHandle) {
    if let Some(p) = PENDING.lock().unwrap().take() {
        copy(&app, &p.result, "");
    }
    close(&app);
}

#[tauri::command]
pub fn preview_close(app: AppHandle) {
    close(&app);
}
