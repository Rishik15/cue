//! Purpose: The tray menu content and what each item does: Settings and Quit. Deliberately short; the rest lives in Settings. Rebuilt on every open.
//! Contents: open — show the menu at the mouse point and run the chosen item; entries — the rows.

use tauri::AppHandle;

use crate::platform::{self, MenuEntry, Point};

const SETTINGS: u32 = 1;
const QUIT: u32 = 2;

fn item(id: u32, label: &str, icon: &'static str) -> MenuEntry {
    MenuEntry::Item { id, label: label.into(), icon: Some(icon) }
}

fn entries() -> Vec<MenuEntry> {
    vec![item(SETTINGS, "Settings…", "settings"), MenuEntry::Separator, item(QUIT, "Quit Cue", "power")]
}

pub fn open(app: &AppHandle, at: Point) {
    match platform::show_menu(entries(), at, &crate::prefs::Prefs::load(app).theme()) {
        Some(SETTINGS) => super::open_settings(app),
        Some(QUIT) => app.exit(0),
        _ => {}
    }
}
