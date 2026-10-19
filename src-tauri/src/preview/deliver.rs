//! Purpose: Gets a finished preview result to where it goes: pasted into the app it came from, or copied to the clipboard (a grab has no app to paste into, and any failure falls back to the clipboard so nothing is lost).
//! Contents: paste — return to the target app and insert the text; copy / copy_with_notice — put text on the clipboard, with a note or the "Copied" pill; notice — show the pill with a message.

use tauri::AppHandle;

use crate::platform::{self, InsertOptions};
use crate::prefs::Prefs;
use crate::status;

/// Own thread: returning focus and typing wait on other programs, and the command must not hold up the window's event loop.
pub(super) fn paste(app: &AppHandle, target: isize, result: String) {
    let (handle, backup) = (app.clone(), result.clone());
    let spawned = crate::guard::spawn("preview-apply", 512 * 1024, move || {
        if !platform::focus_window(target) {
            return copy(&handle, &result, "Cue could not return to the app, so the result was copied to the clipboard.");
        }
        let prefs = Prefs::load(&handle);
        let options = InsertOptions { always_paste: prefs.always_paste, restore_clipboard: prefs.restore_clipboard };
        if let Err(e) = platform::insert(&result, &options) {
            status::report_as(&handle, &e.to_string(), e.pill());
        }
    });
    if let Err(e) = spawned {
        copy(app, &backup, &format!("Could not apply the result ({e}), so it was copied to the clipboard."));
    }
}

/// A grab's result goes to the clipboard, and the pill says so.
pub(super) fn copy_with_notice(app: &AppHandle, text: &str) {
    match platform::copy_text(text) {
        Ok(()) => {
            let prefs = Prefs::load(app);
            platform::overlay::notice("Copied", prefs.overlay_top, prefs.theme());
        }
        Err(e) => status::report(app, &format!("Could not copy the text: {e}")),
    }
}

pub(super) fn copy(app: &AppHandle, text: &str, note: &str) {
    match platform::copy_text(text) {
        Ok(()) if !note.is_empty() => status::report(app, note),
        Ok(()) => {}
        Err(e) => status::report(app, &format!("Could not copy the result: {e}")),
    }
}

/// The native pill with a short message.
pub(super) fn notice(app: &AppHandle, text: &str) {
    let prefs = Prefs::load(app);
    platform::overlay::notice(text, prefs.overlay_top, prefs.theme());
}
