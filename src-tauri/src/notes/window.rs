//! Purpose: The note window's making and placing. It is built hidden with the first note already inside its page (an initialization script), so the editor paints with its text and needs no round trip; the page asks to be shown once it has painted.
//! Contents: exists / is_focused — the window's state; build — create it hidden and place it; show — show it and give it the keyboard; close — remember where it was, then destroy it.
//! The place (x, y, width, height in pixels) is a small file beside the database, not a setting: saving a setting starts the store plugin's runtime, which a feature used all day must not do.

use std::path::PathBuf;

use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewWindow, WebviewWindowBuilder};

use crate::platform;

pub(super) const WINDOW: &str = "notes";
const FRAME_FILE: &str = "notes-frame.txt";

fn window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(WINDOW)
}

pub fn exists(app: &AppHandle) -> bool {
    window(app).is_some()
}

pub fn is_focused(app: &AppHandle) -> bool {
    window(app).is_some_and(|w| w.is_focused().unwrap_or(false))
}

/// Creates the window hidden. `init` is a script that runs before the page and leaves the first note in `window.__NOTE__`.
pub fn build(app: &AppHandle, init: String) -> Result<(), String> {
    let config = app.config().app.windows.iter().find(|w| w.label == WINDOW).ok_or("the window is not configured")?;
    let window =
        WebviewWindowBuilder::from_config(app, config).and_then(|b| b.initialization_script(init).build()).map_err(|e| e.to_string())?;
    place(app, &window);
    Ok(())
}

/// Shown once the page has painted; Windows refuses a plain focus request from a background process, so the platform call does it.
pub fn show(app: &AppHandle) {
    let Some(window) = window(app) else { return };
    let _ = window.show();
    let _ = window.set_always_on_top(true);
    match crate::preview::native_handle(&window) {
        Some(handle) => {
            platform::focus_window(handle);
        }
        None => {
            let _ = window.set_focus();
        }
    }
}

pub fn close(app: &AppHandle) {
    let Some(window) = window(app) else { return };
    if let (Ok(p), Ok(s)) = (window.outer_position(), window.outer_size()) {
        if let Some(path) = frame_path(app) {
            let _ = std::fs::write(path, format!("{},{},{},{}", p.x, p.y, s.width, s.height));
        }
    }
    let _ = window.destroy();
}

fn frame_path(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_data_dir().ok().map(|d| d.join(FRAME_FILE))
}

/// Where it was last time if that is still on a screen, else the middle of the screen the pointer is on.
fn place(app: &AppHandle, window: &WebviewWindow) {
    if let Some((x, y, w, h)) = saved_frame(app) {
        let (cx, cy) = (f64::from(x) + f64::from(w) / 2.0, f64::from(y) + f64::from(h) / 2.0);
        if matches!(app.monitor_from_point(cx, cy), Ok(Some(_))) {
            let _ = window.set_size(PhysicalSize::new(w, h));
            let _ = window.set_position(PhysicalPosition::new(x, y));
            return;
        }
    }
    centre_on_pointer(app, window);
}

fn saved_frame(app: &AppHandle) -> Option<(i32, i32, u32, u32)> {
    let text = std::fs::read_to_string(frame_path(app)?).ok()?;
    let mut parts = text.split(',');
    let (x, y) = (parts.next()?.parse().ok()?, parts.next()?.parse().ok()?);
    let (w, h): (u32, u32) = (parts.next()?.parse().ok()?, parts.next()?.parse().ok()?);
    (parts.next().is_none() && (200..=10_000).contains(&w) && (200..=10_000).contains(&h)).then_some((x, y, w, h))
}

fn centre_on_pointer(app: &AppHandle, window: &WebviewWindow) {
    let monitor = app.cursor_position().ok().and_then(|p| app.monitor_from_point(p.x, p.y).ok().flatten());
    let (Some(monitor), Ok(size)) = (monitor, window.outer_size()) else { return window.center().unwrap_or(()) };
    let area = monitor.work_area();
    let x = area.position.x + (i32::try_from(area.size.width).unwrap_or(0) - i32::try_from(size.width).unwrap_or(0)).max(0) / 2;
    let y = area.position.y + (i32::try_from(area.size.height).unwrap_or(0) - i32::try_from(size.height).unwrap_or(0)).max(0) / 2;
    let _ = window.set_position(PhysicalPosition::new(x, y));
}
