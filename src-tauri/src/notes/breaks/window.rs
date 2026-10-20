//! Purpose: The break screen's window: built hidden with its data inside the page, shown full screen on the monitor under the pointer once the page has painted, destroyed when the rest ends.
//! Contents: exists — is it open; build — create it hidden and send it to the pointer's monitor; show — fill that monitor, float and take the keyboard; close — destroy it.

use tauri::{AppHandle, Manager, WebviewWindow, WebviewWindowBuilder};

use crate::platform;

const WINDOW: &str = "break";

fn window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(WINDOW)
}

pub fn exists(app: &AppHandle) -> bool {
    window(app).is_some()
}

/// `init` is a script that leaves the break's data in `window.__BREAK__`, so the page paints with it and needs no round trip.
pub fn build(app: &AppHandle, init: String) -> Result<(), String> {
    let config = app.config().app.windows.iter().find(|w| w.label == WINDOW).ok_or("the window is not configured")?;
    let window = WebviewWindowBuilder::from_config(app, config)
        .and_then(|b| b.background_color(crate::notes::window::backdrop(app)).initialization_script(init).build())
        .map_err(|e| e.to_string())?;
    let pointer = app.cursor_position().ok().and_then(|p| app.monitor_from_point(p.x, p.y).ok().flatten());
    if let Some(monitor) = pointer.or_else(|| window.current_monitor().ok().flatten()) {
        let _ = window.set_position(*monitor.position());
    }
    Ok(())
}

/// Windows refuses a plain focus request from a background process, so the platform call brings it to the front.
pub fn show(app: &AppHandle) {
    let Some(window) = window(app) else { return };
    let _ = window.set_fullscreen(true);
    let _ = window.show();
    match crate::preview::native_handle(&window) {
        Some(handle) => {
            platform::float_window(handle);
            platform::focus_window(handle);
        }
        None => {
            let _ = window.set_focus();
        }
    }
}

pub fn close(app: &AppHandle) {
    if let Some(window) = window(app) {
        let _ = window.destroy();
    }
}
