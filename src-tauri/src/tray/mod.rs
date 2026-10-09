//! Purpose: Tray icon and settings-window lifecycle. Closing the settings window destroys it (frees the webview);
//! Cue keeps running in the tray so the global shortcut stays active.
//! Contents: init — builds the tray (right click opens the popup menu from `menu`, left click opens settings);
//! open_settings — focuses the window or recreates it from the config.

mod menu;

use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, WebviewWindowBuilder};

use crate::platform::Point;

const WINDOW: &str = "settings";
pub const TRAY_ID: &str = "main";

pub fn open_settings(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(WINDOW) {
        let _ = window.set_focus();
        return;
    }
    if let Some(config) = app.config().app.windows.iter().find(|w| w.label == WINDOW) {
        let _ = WebviewWindowBuilder::from_config(app, config).and_then(|b| b.build());
    }
}

/// The menu runs its own message loop until it closes, so it gets a short-lived thread instead of blocking the tray's
/// window procedure (and the settings window it may open needs a free event loop).
pub fn init(app: &AppHandle) -> tauri::Result<()> {
    let mut tray = TrayIconBuilder::with_id(TRAY_ID).tooltip("Cue").on_tray_icon_event(|tray, event| {
        let TrayIconEvent::Click { button, button_state: MouseButtonState::Up, position, .. } = event else { return };
        let app = tray.app_handle().clone();
        std::thread::spawn(move || match button {
            MouseButton::Left => open_settings(&app),
            MouseButton::Right => menu::open(&app, Point { x: position.x as i32, y: position.y as i32 }),
            _ => {}
        });
    });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}
