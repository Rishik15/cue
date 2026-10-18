//! Purpose: One answer to "is something already using the screen or the keyboard?" for every shortcut press, so two features never read or type at once (a second Ctrl+C from the wrong window, a card over the palette).
//! Contents: busy — dictating, the palette or the question window open, a lookup or card up, a selection or grab in progress.

use tauri::AppHandle;

use crate::{dictation, dictionary, grab, preview, selection, snippets};

pub fn busy(app: &AppHandle) -> bool {
    dictation::is_recording()
        || preview::is_open(app)
        || dictionary::is_looking()
        || selection::is_busy()
        || grab::is_busy()
        || snippets::ask::is_open()
}
