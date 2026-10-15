//! Purpose: Asks for the values of a snippet's `{argument:Label}` placeholders in a small window and waits for the answer. The window is built when asked and destroyed when answered, so it costs nothing at rest.
//! Contents: ask — open the window and block the calling worker thread until the user answers or cancels; cancel — the window went away; ask_get / ask_submit — the page's two commands.

use std::collections::HashMap;
use std::sync::mpsc::{channel, Sender};
use std::sync::Mutex;
use std::time::Duration;

use tauri::{AppHandle, LogicalSize, Manager, WebviewWindowBuilder};

use super::expand::template::Argument;
use crate::{platform, status};

const WINDOW: &str = "ask";
/// A window left open this long is given up on, so a forgotten question never holds the expander.
const PATIENCE: Duration = Duration::from_secs(300);
/// Longest answer accepted for one value.
const MAX_VALUE: usize = 4000;

struct Pending {
    args: Vec<Argument>,
    reply: Sender<Option<Vec<String>>>,
}

static PENDING: Mutex<Option<Pending>> = Mutex::new(None);

/// Shows the questions and waits. None when the user cancels, closes the window or does not answer; otherwise each label with its answer.
pub fn ask(app: &AppHandle, args: Vec<Argument>) -> Option<HashMap<String, String>> {
    let (reply, answer) = channel();
    *PENDING.lock().unwrap() = Some(Pending { args: args.clone(), reply });
    if let Err(e) = open(app, args.len()) {
        PENDING.lock().unwrap().take();
        status::report(app, &format!("Could not ask for the snippet's values: {e}"));
        return None;
    }
    let values = answer.recv_timeout(PATIENCE).ok().flatten();
    PENDING.lock().unwrap().take();
    if let Some(window) = app.get_webview_window(WINDOW) {
        let _ = window.destroy();
    }
    Some(args.into_iter().map(|a| a.label).zip(values?).collect())
}

fn open(app: &AppHandle, count: usize) -> Result<(), String> {
    let config = app.config().app.windows.iter().find(|w| w.label == WINDOW).ok_or("the window is not configured")?;
    let window = WebviewWindowBuilder::from_config(app, config).and_then(|b| b.build()).map_err(|e| e.to_string())?;
    let _ = window.set_size(LogicalSize::new(440.0, 112.0 + 66.0 * count as f64));
    let _ = window.center();
    Ok(())
}

/// The window was destroyed by any means: that is a cancel.
pub fn cancel() {
    if let Some(pending) = PENDING.lock().unwrap().take() {
        let _ = pending.reply.send(None);
    }
}

/// The questions; the page calls this once it is up, and the window then takes the keyboard (the app we came from must not stay above it).
#[tauri::command]
pub fn ask_get(app: AppHandle) -> Option<Vec<Argument>> {
    if let Some(handle) = app.get_webview_window(WINDOW).as_ref().and_then(crate::preview::native_handle) {
        platform::focus_window(handle);
    }
    PENDING.lock().unwrap().as_ref().map(|p| p.args.clone())
}

/// `values` in the order of the questions; None cancels.
#[tauri::command]
pub fn ask_submit(values: Option<Vec<String>>) {
    let Some(pending) = PENDING.lock().unwrap().take() else { return };
    let values =
        values.filter(|v| v.len() == pending.args.len()).map(|v| v.into_iter().map(|s| s.chars().take(MAX_VALUE).collect()).collect());
    let _ = pending.reply.send(values);
}
