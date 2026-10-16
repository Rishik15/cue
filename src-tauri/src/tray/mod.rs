//! Purpose: Tray icon and settings-window lifecycle. Closing the settings window destroys it (frees the webview);
//! Cue keeps running in the tray so the global shortcut stays active.
//! Contents: init — builds the tray (right click opens the popup menu from `menu`, left click opens settings);
//! open_settings — focuses the window or recreates it from the config, after a closing one is gone; note_window_event — tells it when the settings window starts and finishes closing.

mod menu;

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, WebviewWindowBuilder, WindowEvent};

use crate::platform::Point;

const WINDOW: &str = "settings";
pub const TRAY_ID: &str = "main";

/// The settings window was asked to close and is not destroyed yet.
static CLOSING: AtomicBool = AtomicBool::new(false);

/// Called for every window event. Creating a window while the previous settings window is still being destroyed re-enters the event loop inside the
/// runtime's window table and panics ("already borrowed", seen as a crash when reopening at once), so `open_settings` waits for the close to end.
pub fn note_window_event(label: &str, event: &WindowEvent) {
    if label != WINDOW {
        return;
    }
    match event {
        WindowEvent::CloseRequested { .. } => CLOSING.store(true, Ordering::Release),
        WindowEvent::Destroyed => CLOSING.store(false, Ordering::Release),
        _ => {}
    }
}

/// Waits (at most two seconds, only while a close is under way) and then a moment more, so the destroy has fully returned to the event loop. Never call it from the main thread: the close can only finish there.
fn wait_for_close() {
    if !CLOSING.load(Ordering::Acquire) {
        return;
    }
    for _ in 0..100 {
        std::thread::sleep(Duration::from_millis(20));
        if !CLOSING.load(Ordering::Acquire) {
            break;
        }
    }
    std::thread::sleep(Duration::from_millis(60));
}

pub fn open_settings(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(WINDOW) {
        let _ = window.set_focus();
        return;
    }
    wait_for_close();
    if let Some(config) = app.config().app.windows.iter().find(|w| w.label == WINDOW) {
        let _ = WebviewWindowBuilder::from_config(app, config).and_then(|b| b.build());
    }
}

/// The menu runs its own message loop until it closes, so it gets a short-lived thread instead of blocking the tray's
/// window procedure (and the settings window it may open needs a free event loop).
pub fn init(app: &AppHandle) -> tauri::Result<()> {
    let icon = tauri::image::Image::from_bytes(include_bytes!("../../icons/tray.png"))?;
    let tray = TrayIconBuilder::with_id(TRAY_ID).icon(icon).tooltip("Cue").on_tray_icon_event(|tray, event| {
        let TrayIconEvent::Click { button, button_state: MouseButtonState::Up, position, .. } = event else { return };
        let app = tray.app_handle().clone();
        let _ = crate::guard::spawn("tray-click", 512 * 1024, move || match button {
            MouseButton::Left => open_settings(&app),
            MouseButton::Right => menu::open(&app, Point { x: position.x as i32, y: position.y as i32 }),
            _ => {}
        });
    });
    tray.build(app)?;
    Ok(())
}
