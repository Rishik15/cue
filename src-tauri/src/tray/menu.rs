//! Purpose: The tray menu content and what each item does, laid out like Raycast's: Open, Settings, Quit, plus a gray
//! problem row only when something is wrong. Deliberately short; the rest lives in Settings. Rebuilt on every open.
//! Contents: open — show the menu at the mouse point and run the chosen item; entries — the rows.

use tauri::AppHandle;

use crate::platform::{self, MenuEntry, Point};
use crate::status;

const OPEN: u32 = 1;
const SETTINGS: u32 = 2;
const QUIT: u32 = 3;
#[cfg(debug_assertions)]
const TEST_INSERT: u32 = 4;

fn item(id: u32, label: &str, icon: &'static str) -> MenuEntry {
    MenuEntry::Item { id, label: label.into(), icon: Some(icon), enabled: true }
}

/// Shown only when something is wrong (mic missing, insertion blocked): a gray, disabled row so it informs and is no button.
fn problem_rows() -> Vec<MenuEntry> {
    let problem = status::get_status();
    if problem.is_empty() {
        return Vec::new();
    }
    vec![MenuEntry::Separator, MenuEntry::Item { id: 0, label: problem, icon: Some("warn"), enabled: false }]
}

fn entries() -> Vec<MenuEntry> {
    let mut list = vec![item(OPEN, "Open Cue", "app")];
    list.extend(problem_rows());
    list.extend([MenuEntry::Separator, item(SETTINGS, "Settings…", "settings")]);
    #[cfg(debug_assertions)]
    list.push(item(TEST_INSERT, "Test Insert (3 s delay)", "bug"));
    list.extend([MenuEntry::Separator, item(QUIT, "Quit Cue", "power")]);
    list
}

pub fn open(app: &AppHandle, at: Point) {
    match platform::show_menu(entries(), at) {
        Some(OPEN) => crate::dictation::start_from_menu(app), // the command bar will take this over (M1)
        Some(SETTINGS) => super::open_settings(app),
        Some(QUIT) => app.exit(0),
        #[cfg(debug_assertions)]
        Some(TEST_INSERT) => test_insert(),
        _ => {}
    }
}

/// Click, then focus any text field: types a short line, then pastes a multi-line block.
#[cfg(debug_assertions)]
fn test_insert() {
    std::thread::sleep(std::time::Duration::from_secs(3));
    let _ = platform::insert("Cue short insert ✓ ", &Default::default());
    let _ = platform::insert("Cue paste line one\nline two", &Default::default());
}
