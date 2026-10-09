//! Purpose: Tray icon and settings-window lifecycle. Closing the window destroys it (frees the webview);
//! Cue keeps running in the tray so the global shortcut stays active.
//! Contents: init — builds the tray (left click opens settings, menu has Open / Quit);
//! open_settings — focuses the window or recreates it from the config.

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, WebviewWindowBuilder};

const WINDOW: &str = "settings";

pub fn open_settings(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(WINDOW) {
        let _ = window.set_focus();
        return;
    }
    if let Some(config) = app.config().app.windows.first() {
        let _ = WebviewWindowBuilder::from_config(app, config).and_then(|b| b.build());
    }
}

pub fn init(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open Cue", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Cue", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quit])?;
    let mut tray = TrayIconBuilder::new()
        .tooltip("Cue")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => open_settings(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                open_settings(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}
