//! File: deliver.rs
//! Purpose: Gets a finished preview result to where it goes: pasted into the app it came from, or copied to the clipboard (a grab has no app to paste into, and any failure falls back to the clipboard so nothing is lost).
//! Contents: paste — return to the target app and insert the text; copy / copy_with_notice / copy_dictated — put text on the clipboard, with a note or the "Copied" pill (a dictated text whose copy fails is kept in the recovery slot); notice — show the pill with a message.

use tauri::{AppHandle, Manager};

use crate::dictation::recovery;
use crate::platform::{self, InsertOptions, Target};
use crate::prefs::Prefs;
use crate::status;

/// Returns focus to `target` and inserts `result` there, on its own thread; it first waits for the preview window to be gone (`dismiss` only asks for it to be destroyed, and returning focus while it still exists lets its closing take the focus back): returning focus and typing wait on other programs, and the command must not hold up the window's event loop. Focus is checked again at the moment of delivery.
pub(super) fn paste(app: &AppHandle, target: Target, result: String, keep: bool) {
    let (handle, backup) = (app.clone(), result.clone());
    let spawned = crate::guard::spawn("preview-apply", 512 * 1024, move || {
        for _ in 0..50 {
            if handle.get_webview_window("preview").is_none() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        if !platform::focus_window(target.window) {
            return fallback(&handle, &result, "Cue could not return to the app, so the result was copied to the clipboard.", keep);
        }
        let prefs = Prefs::load();
        let options = InsertOptions {
            always_paste: prefs.always_paste,
            restore_clipboard: prefs.restore_clipboard,
            target: target.window,
            token: Some(target),
            ..Default::default()
        };
        match platform::insert(&result, &options) {
            Ok(()) if keep => recovery::clear(),
            Ok(()) => {}
            Err(e) if keep => recovery::report_failure(&handle, &result, &e),
            Err(e) => status::report_as(&handle, &e.to_string(), e.pill()),
        }
    });
    if let Err(e) = spawned {
        fallback(app, &backup, &format!("Could not apply the result ({e}), so it was copied to the clipboard."), keep);
    }
}

/// Copies `text` and shows `note`; a dictated text (`keep`) that cannot be copied is kept in the recovery slot.
fn fallback(app: &AppHandle, text: &str, note: &str, keep: bool) {
    if keep {
        copy_dictated(app, text, note);
    } else {
        copy(app, text, note);
    }
}

/// A grab's result goes to the clipboard, and the pill says so.
pub(super) fn copy_with_notice(app: &AppHandle, text: &str) {
    match platform::copy_text(text) {
        Ok(()) => {
            let prefs = Prefs::load();
            platform::overlay::notice("Copied", prefs.overlay_top, prefs.theme());
        }
        Err(e) => status::report(app, &format!("Could not copy the text: {e}")),
    }
}

/// Copies the text to the clipboard and shows `note` (when there is one), or reports why it could not be copied.
pub(super) fn copy(app: &AppHandle, text: &str, note: &str) {
    match platform::copy_text(text) {
        Ok(()) if !note.is_empty() => status::report(app, note),
        Ok(()) => {}
        Err(e) => status::report(app, &format!("Could not copy the result: {e}")),
    }
}

/// The native pill with a short message.
pub(super) fn notice(text: &str) {
    let prefs = Prefs::load();
    platform::overlay::notice(text, prefs.overlay_top, prefs.theme());
}

/// Copies a dictated text to the clipboard and shows `note`. Dictated text exists nowhere else, so a copy that fails keeps it in the dictation recovery slot and says so.
pub(super) fn copy_dictated(app: &AppHandle, text: &str, note: &str) {
    match platform::copy_text(text) {
        Ok(()) => {
            recovery::clear();
            status::report(app, note);
        }
        Err(e) => {
            let earlier = recovery::hold_note(text);
            status::report(app, &format!("Could not copy the dictated text: {e}. Cue is keeping it for this session.{earlier}"));
        }
    }
}
