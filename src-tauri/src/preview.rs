//! Purpose: The preview window: shows a text before and after a rule changed it, and pastes or drops the result.
//! A webview created when needed and destroyed when closed, so it costs nothing while unused.
//! Contents: show — a finished result; warm — build the window hidden; working / choose / finish — the original is known (and maybe a list to pick from), then the result arrives; preview_choose — the pick; dismiss — close without applying; forget — drop the pending text when the window is gone; preview_get / preview_apply / preview_close — commands for the page.

use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WebviewWindowBuilder};

use crate::platform::{self, InsertOptions};
use crate::prefs::Prefs;
use crate::status;

const WINDOW: &str = "preview";

struct Pending {
    original: String,
    /// None while the actions are still running (the window shows that it is working).
    result: Option<String>,
    /// Name of the action being run, shown in the footer (empty while the user is still choosing).
    via: String,
    /// Its step labels, shown in the Steps field (empty when not known).
    steps: String,
    /// The actions to pick from when more than one matches; None once one is running.
    choices: Option<Vec<Choice>>,
    /// The window the text came from; Apply brings it back to the front before inserting.
    target: isize,
}

static PENDING: Mutex<Option<Pending>> = Mutex::new(None);

#[derive(Serialize, Clone)]
pub struct Choice {
    at: usize,
    name: String,
    steps: String,
}

#[derive(Serialize)]
pub struct View {
    original: String,
    result: Option<String>,
    via: String,
    steps: String,
    choices: Option<Vec<Choice>>,
}

/// Builds the window hidden, unfocused and unfocusable (the page shows it without taking focus until the text is copied). The webview is the slow part (a quarter of a second), so this starts while the selection is still being
/// copied; the page shows the window itself as soon as there is something to show.
pub fn warm(app: &AppHandle) {
    if app.get_webview_window(WINDOW).is_some() {
        return;
    }
    let Some(config) = app.config().app.windows.iter().find(|w| w.label == WINDOW) else { return };
    // Created from the config thread-safely: the caller is a worker thread, never the event loop.
    if let Err(e) = WebviewWindowBuilder::from_config(app, config).and_then(|b| b.focused(false).focusable(false).build()) {
        status::report(app, &format!("Could not open the preview: {e}"));
    }
}

/// The text is known and the actions are running: the window shows the original and a working state.
pub fn working(app: &AppHandle, original: String, (via, steps): (String, String), target: isize) {
    *PENDING.lock().unwrap() = Some(Pending { original, result: None, via, steps, choices: None, target });
    warm(app);
    let _ = app.emit_to(WINDOW, "preview-changed", ());
    take_focus(app);
}

/// The text is copied, so the window may take the keyboard now (Escape, Enter, arrows).
fn take_focus(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(WINDOW) {
        let _ = window.set_focusable(true);
        let _ = window.set_focus();
    }
}

/// More than one action matches: the window lists them and waits for the user's pick (`preview_choose`).
pub fn choose(app: &AppHandle, original: String, found: Vec<crate::rules::Candidate>, target: isize) {
    let choices = found.into_iter().map(|c| Choice { at: c.at, name: c.name, steps: c.steps }).collect();
    *PENDING.lock().unwrap() =
        Some(Pending { original, result: None, via: String::new(), steps: String::new(), choices: Some(choices), target });
    warm(app);
    let _ = app.emit_to(WINDOW, "preview-changed", ());
    take_focus(app);
}

pub fn is_open(app: &AppHandle) -> bool {
    app.get_webview_window(WINDOW).is_some()
}

/// The actions finished: the result replaces the working state.
pub fn finish(app: &AppHandle, result: String) {
    if let Some(p) = PENDING.lock().unwrap().as_mut() {
        p.result = Some(result);
    }
    let _ = app.emit_to(WINDOW, "preview-changed", ());
}

/// A finished result at once (dictation: the text is ready before any window exists).
pub fn show(app: &AppHandle, original: String, result: String, via: String, target: isize) {
    working(app, original, (via, String::new()), target);
    finish(app, result);
}

/// Closes the window without applying anything (nothing to review after all, or the flow was cancelled).
pub fn dismiss(app: &AppHandle) {
    close(app);
}

/// The window was closed by any means (its own buttons, Alt+F4): the text must not linger in memory.
pub fn forget() {
    PENDING.lock().unwrap().take();
}

fn close(app: &AppHandle) {
    PENDING.lock().unwrap().take();
    if let Some(window) = app.get_webview_window(WINDOW) {
        let _ = window.destroy();
    }
}

#[tauri::command]
pub fn preview_get() -> Option<View> {
    PENDING.lock().unwrap().as_ref().map(|p| View {
        original: p.original.clone(),
        result: p.result.clone(),
        via: p.via.clone(),
        steps: p.steps.clone(),
        choices: p.choices.clone(),
    })
}

/// The pending text once its result is ready; while the actions still run, Paste does nothing (the window stays).
fn take_ready() -> Option<(Pending, String)> {
    let mut pending = PENDING.lock().unwrap();
    let result = pending.as_ref()?.result.clone()?;
    Some((pending.take()?, result))
}

#[tauri::command]
pub fn preview_apply(app: AppHandle) {
    let Some((p, result)) = take_ready() else { return };
    close(&app);
    // Own thread: returning focus and typing wait on other programs, and the command must not hold up the window's event loop.
    let (handle, backup) = (app.clone(), result.clone());
    let spawned = std::thread::Builder::new().name("preview-apply".into()).stack_size(512 * 1024).spawn(move || {
        if !platform::focus_window(p.target) {
            return copy(&handle, &result, "Cue could not return to the app, so the result was copied to the clipboard.");
        }
        let prefs = Prefs::load(&handle);
        let options = InsertOptions { always_paste: prefs.always_paste, restore_clipboard: prefs.restore_clipboard };
        if let Err(e) = platform::insert(&result, &options) {
            status::report(&handle, &e.to_string());
        }
    });
    if let Err(e) = spawned {
        copy(&app, &backup, &format!("Could not apply the result ({e}), so it was copied to the clipboard."));
    }
}

fn copy(app: &AppHandle, text: &str, note: &str) {
    match platform::copy_text(text) {
        Ok(()) if !note.is_empty() => status::report(app, note),
        Ok(()) => {}
        Err(e) => status::report(app, &format!("Could not copy the result: {e}")),
    }
}

/// The user picked an action from the list: it runs on the text the window holds.
#[tauri::command]
pub fn preview_choose(app: AppHandle, at: usize) {
    let picked = PENDING.lock().unwrap().as_ref().filter(|p| p.choices.is_some()).map(|p| (p.original.clone(), p.target));
    if let Some((text, target)) = picked {
        crate::selection::chosen(&app, text, target, at);
    }
}

/// Cancel: also stops the selection flow, so a result that arrives later opens nothing.
#[tauri::command]
pub fn preview_close(app: AppHandle) {
    crate::selection::cancel();
    close(&app);
}
