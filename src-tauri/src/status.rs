//! Purpose: Surface problems to the user without a window: the tray tooltip shows the latest one until things work again.
//! Contents: report — show a problem (tooltip + `cue-status` event for open windows); clear — back to normal;
//! get_status — latest problem, shown by the tray menu.

use std::sync::Mutex;

use tauri::{AppHandle, Emitter};

use crate::tray::TRAY_ID;

const IDLE: &str = "Cue";

static LAST: Mutex<String> = Mutex::new(String::new());

fn remember(text: &str) {
    *LAST.lock().unwrap() = text.to_string();
}

pub fn get_status() -> String {
    LAST.lock().unwrap().clone()
}

fn set_tooltip(app: &AppHandle, text: &str) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_tooltip(Some(text));
    }
}

pub fn report(app: &AppHandle, problem: &str) {
    eprintln!("cue: {problem}");
    remember(problem);
    set_tooltip(app, &format!("Cue: {problem}"));
    let _ = app.emit("cue-status", problem);
}

pub fn clear(app: &AppHandle) {
    if LAST.lock().unwrap().is_empty() {
        return; // nothing shown: skip the round trip to the main thread (this runs on the hotkey thread)
    }
    remember("");
    set_tooltip(app, IDLE);
    let _ = app.emit("cue-status", "");
}
