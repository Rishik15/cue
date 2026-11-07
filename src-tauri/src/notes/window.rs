//! File: window.rs
//! Purpose: The note window's making and placing. It is built hidden with the first note already inside its page (an initialization script), so the editor paints with its text and needs no round trip, and is shown once that page has painted. Its size is fixed and follows the screen.
//! Contents: exists / is_focused — the window's state; build — create it hidden, size it for the screen and place it; show — show it and give it the keyboard; close — remember where it was, then destroy it.
//! The place (x, y in pixels) is a small file beside the database, not a setting: saving a setting starts the store plugin's runtime, which a feature used all day must not do.

use std::path::PathBuf;

use tauri::window::Color;
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewWindow, WebviewWindowBuilder};

use crate::platform;

pub(super) const WINDOW: &str = "notes";
const FRAME_FILE: &str = "notes-frame.txt";

/// The note window if it exists. Input: the app handle. Return: the window or `None`.
fn window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(WINDOW)
}

/// Whether the note window exists. Input: the app handle. Return: true while it is open or hidden.
pub fn exists(app: &AppHandle) -> bool {
    window(app).is_some()
}

/// Whether the note window holds the foreground, by the system's own answer (the toolkit's focus flag goes stale when another program takes the front). Input: the app handle. Return: true when it is the foreground window.
pub fn is_focused(app: &AppHandle) -> bool {
    window(app).and_then(|w| crate::preview::native_handle(&w)).is_some_and(|h| platform::foreground_window() == h)
}

/// The card's own colour while the page is still loading, so the window can be shown the moment it exists: the same two colours as `notes.css`, picked the way Cue picks light or dark.
pub(super) fn backdrop(app: &AppHandle) -> Color {
    let prefs = crate::prefs::Prefs::load(app);
    let dark = prefs.theme_mode == "Dark" || (prefs.theme_mode != "Light" && platform::system_uses_dark_theme());
    if dark {
        Color(0x1c, 0x1c, 0x1e, 255)
    } else {
        Color(0xff, 0xff, 0xff, 255)
    }
}

/// Creates the window hidden, sized for its screen and painted in the note's colour; the page shows it (`Cmd::Ready`) once it has painted. Shown earlier, a second window of the same process came up blank. Its first note travels inside the page as `init`, a script that leaves it in `window.__NOTE__`.
pub fn build(app: &AppHandle, init: String) -> Result<(), String> {
    let config = app.config().app.windows.iter().find(|w| w.label == WINDOW).ok_or("the window is not configured")?;
    let window = WebviewWindowBuilder::from_config(app, config)
        .and_then(|b| b.background_color(Color(0, 0, 0, 0)).initialization_script(init).build())
        .map_err(|e| e.to_string())?;
    place(app, &window);
    Ok(())
}

/// Brings the window to the front and gives it the keyboard; Windows refuses a plain focus request from a background process, so the platform call does it.
/// Shows the window and gives it the foreground, floating and focused. Input: the app handle.
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

/// Gives the page the keyboard: the window holds the OS focus (from `show`), but the web view inside needs its own, or the editor has no caret.
pub fn focus(app: &AppHandle) {
    if let Some(window) = window(app) {
        let _ = AsRef::<tauri::Webview>::as_ref(&window).set_focus();
    }
}

/// Remembers where the window is, then destroys it. Input: the app handle.
pub fn close(app: &AppHandle) {
    crate::platform::stop_clipboard_watch(); // nothing is collecting once the window is gone
    let Some(window) = window(app) else { return };
    if let (Ok(p), Some(path)) = (window.outer_position(), frame_path(app)) {
        let _ = std::fs::write(path, format!("{},{}", p.x, p.y));
    }
    let _ = window.destroy();
}

/// The file that keeps the window's place. Input: the app handle. Return: its path, or `None` with no data folder.
fn frame_path(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_data_dir().ok().map(|d| d.join(FRAME_FILE))
}

/// The window's size never changes (the page has no resize handle): the base size grows with the screen, 1 on a 1000 px high work area up to 1.5, and the page is zoomed by the same factor so its text keeps its proportions.
const BASE: (f64, f64) = (370.0, 425.0);
/// The gap, in dip, between the first-time position and the screen's left and bottom edges.
const EDGE: f64 = 48.0;

/// Moves `wanted` (or, with none, the right edge, vertically centred) so a window of `size` lies fully inside the work area. Input: the area's position and size, the window's size, the saved spot and the margin, all in pixels. Return: the top-left corner; an area smaller than the window pins it to the area's top-left.
fn fit(area: ((i32, i32), (u32, u32)), size: (u32, u32), wanted: Option<(i32, i32)>, margin: i32) -> (i32, i32) {
    let ((ax, ay), (aw, ah)) = area;
    let (w, h) = (i32::try_from(size.0).unwrap_or(0), i32::try_from(size.1).unwrap_or(0));
    let (right, bottom) = (ax + i32::try_from(aw).unwrap_or(0), ay + i32::try_from(ah).unwrap_or(0));
    let (x, y) = wanted.unwrap_or((right - w - margin, ay + (bottom - ay - h) / 2));
    (x.min(right - w).max(ax), y.min(bottom - h).max(ay))
}

/// Where it was last time if that is still on a screen, else on the screen the pointer is on. The size follows that screen's height and scale. The window is moved to its screen first and sized after, so a screen with another scale cannot resize it on the way, and the position is set again after the size in case that move shifted it. The size never exceeds the work area (the page zoom shrinks with it), so it always ends fully inside it.
fn place(app: &AppHandle, window: &WebviewWindow) {
    let saved = saved_position(app).filter(|&(x, y)| matches!(app.monitor_from_point(f64::from(x), f64::from(y)), Ok(Some(_))));
    let at = saved.and_then(|(x, y)| app.monitor_from_point(f64::from(x), f64::from(y)).ok().flatten());
    let pointer = || app.cursor_position().ok().and_then(|p| app.monitor_from_point(p.x, p.y).ok().flatten());
    let Some(monitor) = at.or_else(pointer) else { return };
    let dpi = monitor.scale_factor();
    let area = monitor.work_area();
    let (area_w, area_h) = (f64::from(area.size.width) / dpi, f64::from(area.size.height) / dpi);
    let grow = (area_h / 1000.0).clamp(1.0, 1.5).min(area_h / BASE.1).min(area_w / BASE.0);
    let size = ((BASE.0 * grow * dpi).round() as u32, (BASE.1 * grow * dpi).round() as u32);
    let (x, y) = fit(((area.position.x, area.position.y), (area.size.width, area.size.height)), size, saved, (EDGE * dpi).round() as i32);
    let _ = window.set_position(PhysicalPosition::new(x, y));
    let _ = window.set_size(PhysicalSize::new(size.0, size.1));
    let _ = window.set_position(PhysicalPosition::new(x, y));
    let _ = window.set_zoom(grow);
}

/// The remembered place. Input: the app handle. Return: the saved top-left in pixels, or `None` for a missing or malformed file.
fn saved_position(app: &AppHandle) -> Option<(i32, i32)> {
    let text = std::fs::read_to_string(frame_path(app)?).ok()?;
    let (x, y) = text.split_once(',')?;
    Some((x.trim().parse().ok()?, y.trim().parse().ok()?))
}

#[cfg(test)]
mod tests {
    use super::fit;

    const AREA: ((i32, i32), (u32, u32)) = ((0, 0), (1920, 1040));

    /// With no saved spot the window sits on the right, vertically centred.
    #[test]
    fn first_time_sits_right_and_centred() {
        assert_eq!(fit(AREA, (370, 425), None, 48), (1502, 307));
    }

    /// A saved spot that fits is kept as it is.
    #[test]
    fn saved_spot_inside_is_kept() {
        assert_eq!(fit(AREA, (370, 425), Some((100, 200)), 48), (100, 200));
    }

    /// A saved spot that hangs off the area, including one on a screen to the left, is pulled inside.
    #[test]
    fn saved_spot_hanging_off_is_pulled_in() {
        assert_eq!(fit(AREA, (370, 425), Some((1800, 900)), 48), (1550, 615));
        assert_eq!(fit(((-1920, 0), (1920, 1040)), (370, 425), Some((-2100, -50)), 48), (-1920, 0));
    }

    /// An area smaller than the window pins it to the top-left of the area.
    #[test]
    fn area_smaller_than_window_pins_to_corner() {
        assert_eq!(fit(((10, 20), (300, 300)), (370, 425), Some((50, 50)), 48), (10, 20));
    }
}
