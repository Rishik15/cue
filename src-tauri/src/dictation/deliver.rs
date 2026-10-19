//! Purpose: Delivers a finished dictation: cleaning and rules on their own thread, then typing or pasting at the cursor, or the clipboard when that cannot be done. The text is never dropped.
//! Contents: insert_off_thread — the entry from the speech thread (with the length and window of that dictation); insert — clean, run the rules, route (preview, copy or insert); deliver — where the text goes; insert_text — the insert thread (context fit, typing or pasting, submit key); rescue — copy after a failure.

use tauri::AppHandle;

use super::{clean, context, is_recording, Spoken};
use crate::platform::{self, overlay, InsertOptions};
use crate::prefs::Prefs;
use crate::rules::model::{Output, Trigger};
use crate::{rules, status};

/// Cleaning, rules (which may wait for the text model for a long time) and delivery run on their own thread, so the speech thread keeps serving the next dictation. A panic in them
/// still delivers what was said, to the clipboard.
pub(super) fn insert_off_thread(app: &AppHandle, text: String, spoken: Spoken) {
    let (handle, backup) = (app.clone(), text.clone());
    let spawned = crate::guard::spawn("dictation-rules", 512 * 1024, move || {
        let (h, kept) = (handle.clone(), text.clone());
        if crate::guard::contain(move || insert(&h, kept, spoken)).is_none() {
            rescue(&handle, &text);
        }
    });
    if spawned.is_err() {
        rescue(app, &backup);
    }
}

fn rescue(app: &AppHandle, text: &str) {
    let _ = platform::copy_text(text);
    status::report(app, "Something went wrong while inserting, so the text was copied to the clipboard. Paste it with Ctrl+V.");
}

/// Types the text where the cursor is. The text is never lost: if focus moved to another window, or typing fails for any reason, it goes to
/// the clipboard and the user is told, so it never lands in the wrong app and never vanishes.
fn insert(app: &AppHandle, text: String, spoken: Spoken) {
    let prefs = Prefs::load(app);
    let text = clean::clean(&text, &prefs.vocabulary, prefs.remove_fillers);
    if text.is_empty() {
        // Silence, noise, or only filler sounds: say so, or the user cannot tell this from a failure.
        if is_recording() {
            return; // the next dictation owns the overlay; a notice would replace its pill
        }
        return overlay::notice("Didn't catch any speech", prefs.overlay_top, prefs.theme());
    }
    crate::stats::record(app, text.split_whitespace().count() as u32, spoken.ms as f32 / 1000.0);
    let routed = rules::run(Trigger::Dictation, &text);
    let (original, text, output, via) = (text, routed.text, routed.output, routed.matched.join(", "));
    if output == Output::Preview {
        return crate::preview::show(app, original, text, via, spoken.target);
    }
    deliver(app, text, output == Output::Copy || prefs.output_copy, &prefs, spoken.target);
}

/// Whether focus is still in the window dictation started in (or cannot be told).
fn on_target(target: isize) -> bool {
    let now = platform::foreground_window();
    target == 0 || now == 0 || target == now
}

/// Where the text goes: the clipboard when asked (a rule's output or the Output Action setting) or when focus moved to another window since
/// dictation started (so it never lands in the wrong app), otherwise into the focused app.
fn deliver(app: &AppHandle, text: String, copy: bool, prefs: &Prefs, target: isize) {
    if !on_target(target) {
        match platform::copy_text(&text) {
            Ok(()) => status::report(app, "You switched windows, so the text was copied to the clipboard. Paste it with Ctrl+V."),
            Err(e) => status::report(app, &format!("You switched windows and the text could not be copied: {e}")),
        }
        return;
    }
    if copy {
        match platform::copy_text(&text) {
            Ok(()) => overlay::notice("Copied to clipboard", prefs.overlay_top, prefs.theme()),
            Err(e) => status::report(app, &format!("Could not copy the text: {e}")),
        }
        return;
    }
    let options = InsertOptions { always_paste: prefs.always_paste, restore_clipboard: prefs.restore_clipboard };
    let (handle, context, submit, backup) = (app.clone(), prefs.context_paste, prefs.auto_submit.clone(), text.clone());
    let keep = prefs.keep_copy;
    // Own thread: insertion can wait a few seconds for modifier keys to be released, and must not hold up the speech thread.
    let spawned = crate::guard::spawn("insert", 512 * 1024, move || {
        insert_text(&handle, text, &options, (context, keep), &submit);
    });
    if spawned.is_err() {
        let _ = platform::copy_text(&backup); // the text is never lost, whatever fails
        status::report(app, "Could not type the text, so it is on your clipboard. Paste it with Ctrl+V.");
    }
}

/// Runs on the insert thread. `(context, keep)`: Match Surrounding Text and Insert and Copy. Context-Aware Paste reads the few characters before the cursor and fits the text to them; Auto Submit then
/// presses its key. Both are off by default, and the submit key is only pressed after the text really went in.
fn insert_text(app: &AppHandle, text: String, options: &InsertOptions, (context, keep): (bool, bool), submit: &str) {
    let text = match context.then(platform::text_before_cursor).flatten() {
        Some(before) => self::context::fit(&text, &before),
        None => text,
    };
    if let Err(e) = platform::insert(&format!("{text} "), options) {
        status::report_as(app, &e.to_string(), e.pill());
        // The text may still have gone in (only the clipboard could not be restored): the copy and the submit key then still apply.
        if !e.text_went_in() {
            return;
        }
    }
    // "Insert and Copy": the clipboard is set after the paste has finished (and any restore), so the text is what stays on it.
    if keep && platform::copy_text(&text).is_err() {
        status::report(app, "The text was inserted but could not be copied to the clipboard.");
    }
    let result = match submit {
        "Enter" => platform::press_submit(false),
        "Ctrl+Enter" => platform::press_submit(true),
        _ => Ok(()),
    };
    if let Err(e) = result {
        status::report(app, &format!("Could not press the submit key: {e}"));
    }
}
