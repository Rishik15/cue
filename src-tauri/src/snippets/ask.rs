//! Purpose: Asks for the values of a snippet's `{argument:Label}` placeholders in a small window and waits for the answer. The window is built when asked and destroyed when answered, so it costs nothing at rest.
//! Contents: ask — open the window and block the calling worker thread until the user answers or cancels; claim — register the one open question unless the asker is stale; cancel — the window went away; close — the expander stopped, so the open question is dropped; ask_get / ask_submit — the page's two commands.

use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;
use std::time::Duration;

use tauri::{AppHandle, LogicalSize, Manager, PhysicalPosition, PhysicalSize, WebviewWindowBuilder};

use super::expand::template::Argument;
use crate::{platform, status};

const WINDOW: &str = "ask";
/// Space kept between the window and the screen edge, in logical pixels.
const EDGE_DIP: f64 = 16.0;
/// A window left open this long is given up on, so a forgotten question never holds the expander.
const PATIENCE: Duration = Duration::from_secs(300);
/// Longest answer accepted for one value.
const MAX_VALUE: usize = 4000;

struct Pending {
    args: Vec<Argument>,
    reply: Sender<Option<Vec<String>>>,
}

static PENDING: Mutex<Option<Pending>> = Mutex::new(None);

/// Registers a question unless one is open or `live` says the asker is stale. `live` is read under the slot's lock, and `close` takes that lock after the expander is marked stopped, so a stop either finds this question and drops it or is seen here: none is left open by a worker that was dispatched before the stop. Input: the questions and the liveness check. Returns the receiver of the answer, None when refused.
pub(in crate::snippets) fn claim(args: &[Argument], live: &dyn Fn() -> bool) -> Option<Receiver<Option<Vec<String>>>> {
    let (reply, answer) = channel();
    // One question at a time: a second would replace the first's reply channel and leave that worker waiting for nothing.
    let mut slot = PENDING.lock().unwrap_or_else(|e| e.into_inner());
    if slot.is_some() || !live() {
        return None;
    }
    *slot = Some(Pending { args: args.to_vec(), reply });
    Some(answer)
}

/// Shows the questions and waits. None when the user cancels, closes the window or does not answer, another question is already open, or `live` is false (the expander stopped); otherwise each label with its answer.
pub fn ask(app: &AppHandle, args: Vec<Argument>, live: &dyn Fn() -> bool) -> Option<HashMap<String, String>> {
    let answer = claim(&args, live)?;
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

/// Builds the question window for `count` values, sized to fit and centred in the work area of the monitor the pointer is on (the page scrolls when the window is cut short). Input: the app handle and the number of questions. Return: an error text if the window could not be built.
fn open(app: &AppHandle, count: usize) -> Result<(), String> {
    let config = app.config().app.windows.iter().find(|w| w.label == WINDOW).ok_or("the window is not configured")?;
    crate::runtime::ensure();
    let window = WebviewWindowBuilder::from_config(app, config).and_then(|b| b.build()).map_err(|e| e.to_string())?;
    let wanted = (440.0, 112.0 + 66.0 * count as f64);
    let Some((area, scale)) = platform::work_area_of(0) else {
        let _ = window.set_size(LogicalSize::new(wanted.0, wanted.1));
        let _ = window.center();
        return Ok(());
    };
    let scale = f64::from(scale);
    let (w, h) = platform::place::fit_logical(wanted, area, scale, EDGE_DIP);
    let (pw, ph) = ((w * scale).round() as i32, (h * scale).round() as i32);
    let (x, y) = (area.left + (area.right - area.left - pw) / 2, area.top + (area.bottom - area.top - ph) / 2);
    let _ = window.set_position(PhysicalPosition::new(x.max(area.left), y.max(area.top)));
    let _ = window.set_size(PhysicalSize::new(pw.max(1) as u32, ph.max(1) as u32));
    let _ = window.set_position(PhysicalPosition::new(x.max(area.left), y.max(area.top)));
    Ok(())
}

/// The question window is up or about to be.
pub fn is_open() -> bool {
    PENDING.lock().unwrap_or_else(|e| e.into_inner()).is_some()
}

/// The window was destroyed by any means: that is a cancel, unless a newer question already has its own window (the event of the old one arrives late and must not cancel it).
pub fn cancel(app: &AppHandle) {
    if app.get_webview_window(WINDOW).is_some() {
        return;
    }
    if let Some(pending) = PENDING.lock().unwrap().take() {
        let _ = pending.reply.send(None);
    }
}

/// Drops the open question, if any: its waiting worker gets a cancel and destroys the window. Called when the expander stops.
pub fn close() {
    if let Some(pending) = PENDING.lock().unwrap_or_else(|e| e.into_inner()).take() {
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
