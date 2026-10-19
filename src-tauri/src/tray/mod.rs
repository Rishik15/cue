//! Purpose: Tray icon and settings-window lifecycle. Closing the settings window destroys it (frees the webview);
//! Cue keeps running in the tray so the global shortcut stays active.
//! Contents: init — builds the tray (right click opens the popup menu from `menu`, left click opens settings);
//! open_settings / open_settings_at — focus the window or recreate it from the config, after a closing one is gone (with an update waiting, every open lands on About); take_start_page — the page a new window opens on; note_window_event — tells it when the settings window starts and finishes closing.

mod menu;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, WebviewWindowBuilder, WindowEvent};

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

/// The page a window that is being created opens on (taken once by the page: `take_start_page`).
static START_PAGE: Mutex<Option<&'static str>> = Mutex::new(None);
#[tauri::command]
pub fn take_start_page() -> Option<&'static str> {
    START_PAGE.lock().unwrap().take()
}

/// Opening the settings while an update waits starts on About, until it is installed.
pub fn open_settings(app: &AppHandle) {
    open_settings_at(app, crate::update::ready_version().is_some().then_some("about"));
}

/// Focuses the window or creates it; `page` is where it should be (a window that is already open is told, a new one asks on start).
pub fn open_settings_at(app: &AppHandle, page: Option<&'static str>) {
    if let Some(page) = page {
        *START_PAGE.lock().unwrap() = Some(page);
    }
    if let Some(window) = app.get_webview_window(WINDOW) {
        if let Some(page) = page {
            let _ = app.emit_to(WINDOW, "cue-goto", page);
        }
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
