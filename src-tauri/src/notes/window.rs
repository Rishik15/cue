//! Purpose: The note window's making and placing. It is built with the first note already inside its page (an initialization script), so the editor paints with its text and needs no round trip, and is shown at once in the note's colour. Its size is fixed and follows the screen.
//! Contents: exists / is_focused — the window's state; build — create it, size it for the screen, place it and show it; show — show it and give it the keyboard; close — remember where it was, then destroy it.
//! The place (x, y in pixels) is a small file beside the database, not a setting: saving a setting starts the store plugin's runtime, which a feature used all day must not do.

use std::path::PathBuf;

use tauri::window::Color;
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

/// The card's own colour while the page is still loading, so the window can be shown the moment it exists: the same two colours as `notes.css`, picked the way Cue picks light or dark.
fn backdrop(app: &AppHandle) -> Color {
    let prefs = crate::prefs::Prefs::load(app);
    let dark = prefs.theme_mode == "Dark" || (prefs.theme_mode != "Light" && platform::system_uses_dark_theme());
    if dark {
        Color(0x1c, 0x1c, 0x1e, 255)
    } else {
        Color(0xff, 0xff, 0xff, 255)
    }
}

/// Creates the window, sized for its screen, painted in the note's colour, and shows it at once: the page fills in a moment later (its first note travels inside it as `init`, a script that leaves it in `window.__NOTE__`).
pub fn build(app: &AppHandle, init: String) -> Result<(), String> {
    let config = app.config().app.windows.iter().find(|w| w.label == WINDOW).ok_or("the window is not configured")?;
    let window = WebviewWindowBuilder::from_config(app, config)
        .and_then(|b| b.background_color(backdrop(app)).initialization_script(init).build())
        .map_err(|e| e.to_string())?;
    place(app, &window);
    show(app);
    Ok(())
}

/// Shown once the page has painted; Windows refuses a plain focus request from a background process, so the platform call does it.
pub fn show(app: &AppHandle) {
    let Some(window) = window(app) else { return };
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

/// Gives the page the keyboard: the window was shown before its page existed, and a web view only takes focus once it has content.
pub fn focus(app: &AppHandle) {
    if let Some(window) = window(app) {
        let _ = window.set_focus();
    }
}

pub fn close(app: &AppHandle) {
    crate::platform::stop_clipboard_watch(); // nothing is collecting once the window is gone
    let Some(window) = window(app) else { return };
    if let (Ok(p), Some(path)) = (window.outer_position(), frame_path(app)) {
        let _ = std::fs::write(path, format!("{},{}", p.x, p.y));
    }
    let _ = window.destroy();
}

fn frame_path(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_data_dir().ok().map(|d| d.join(FRAME_FILE))
}

/// The window's size never changes (the page has no resize handle): the base size grows with the screen, 1 on a 1000 px high work area up to 1.5, and the page is zoomed by the same factor so its text keeps its proportions.
const BASE: (f64, f64) = (320.0, 368.0);

/// Where it was last time if that is still on a screen, else the middle of the screen the pointer is on.
fn place(app: &AppHandle, window: &WebviewWindow) {
    let saved = saved_position(app).filter(|&(x, y)| matches!(app.monitor_from_point(f64::from(x), f64::from(y)), Ok(Some(_))));
    let at = saved.and_then(|(x, y)| app.monitor_from_point(f64::from(x), f64::from(y)).ok().flatten());
    let pointer = || app.cursor_position().ok().and_then(|p| app.monitor_from_point(p.x, p.y).ok().flatten());
    let Some(monitor) = at.or_else(pointer) else { return };
    let dpi = monitor.scale_factor();
    let area = monitor.work_area();
    let grow = (f64::from(area.size.height) / dpi / 1000.0).clamp(1.0, 1.5);
    let (w, h) = ((BASE.0 * grow * dpi).round() as u32, (BASE.1 * grow * dpi).round() as u32);
    let _ = window.set_size(PhysicalSize::new(w, h));
    let _ = window.set_zoom(grow);
    let (x, y) = saved.unwrap_or_else(|| {
        let middle =
            |origin: i32, space: u32, own: u32| origin + (i32::try_from(space).unwrap_or(0) - i32::try_from(own).unwrap_or(0)).max(0) / 2;
        (middle(area.position.x, area.size.width, w), middle(area.position.y, area.size.height, h))
    });
    let _ = window.set_position(PhysicalPosition::new(x, y));
}

fn saved_position(app: &AppHandle) -> Option<(i32, i32)> {
    let text = std::fs::read_to_string(frame_path(app)?).ok()?;
    let (x, y) = text.split_once(',')?;
    Some((x.trim().parse().ok()?, y.trim().parse().ok()?))
}
