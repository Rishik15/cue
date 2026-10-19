//! Purpose: The tray menu content and what each item does: Settings and Quit, plus Update Cue while an update waits. Deliberately short; the rest lives in Settings. Rebuilt on every open.
//! Contents: open — show the menu at the mouse point and run the chosen item; entries — the rows.

use tauri::AppHandle;

use crate::platform::{self, MenuEntry, Point};

const SETTINGS: u32 = 1;
const QUIT: u32 = 2;
const UPDATE: u32 = 3;

fn item(id: u32, label: &str, icon: &'static str) -> MenuEntry {
    MenuEntry::Item { id, label: label.into(), icon: Some(icon) }
}

fn entries(update_waiting: bool) -> Vec<MenuEntry> {
    let update = update_waiting.then(|| [item(UPDATE, "Update Cue", "app"), MenuEntry::Separator]).into_iter().flatten();
    update.chain([item(SETTINGS, "Settings…", "settings"), MenuEntry::Separator, item(QUIT, "Quit Cue", "power")]).collect()
}

pub fn open(app: &AppHandle, at: Point) {
    match platform::show_menu(entries(crate::update::ready_version().is_some()), at, &crate::prefs::Prefs::load(app).theme()) {
        Some(UPDATE) => super::open_settings_at(app, Some("about")),
        Some(SETTINGS) => super::open_settings(app),
        Some(QUIT) => app.exit(0),
        _ => {}
    }
}
