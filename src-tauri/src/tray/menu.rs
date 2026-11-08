//! File: menu.rs
//! Purpose: The tray menu content and what each item does: Settings and Quit. Deliberately short; the rest lives in Settings. Rebuilt on every open.
//! Contents: open — show the menu at the mouse point and run the chosen item, reporting a menu that could not open; entries — the rows.

use tauri::AppHandle;

use crate::platform::{self, MenuEntry, Point};

const SETTINGS: u32 = 1;
const QUIT: u32 = 2;

/// One menu row. Input: its id, label and icon name. Return: the entry.
fn item(id: u32, label: &str, icon: &'static str) -> MenuEntry {
    MenuEntry::Item { id, label: label.into(), icon: Some(icon) }
}

/// The rows of the tray menu.
fn entries() -> Vec<MenuEntry> {
    vec![item(SETTINGS, "Settings…", "settings"), MenuEntry::Separator, item(QUIT, "Quit Cue", "power")]
}

/// Shows the menu at `at` and runs the chosen item. A dismissed menu does nothing; one that could not open is reported to the user.
pub fn open(app: &AppHandle, at: Point) {
    match platform::show_menu(entries(), at, &crate::prefs::Prefs::load().theme()) {
        Ok(Some(SETTINGS)) => super::open_settings(app),
        Ok(Some(QUIT)) => app.exit(0),
        Ok(_) => {}
        Err(e) => crate::status::report(app, &format!("The tray menu could not open: {e}.")),
    }
}
