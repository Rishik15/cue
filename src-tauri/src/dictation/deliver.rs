//! File: deliver.rs
//! Purpose: Delivers a finished dictation: cleaning and rules on their own thread, then typing or pasting at the cursor, or the clipboard when that cannot be done. The text is never dropped.
//! Contents: insert_off_thread — the entry from the speech thread (with the length and window of that dictation); insert — clean, run the rules, route (preview, copy or insert); deliver — where the text goes; insert_text — the insert thread (context fit, then one platform delivery that checks the target window, types or pastes, copies and presses the submit key); rescue — copy after a failure; submit_of — the setting as a key.

use tauri::AppHandle;

use super::{clean, context, is_recording, Spoken};
use crate::platform::{self, overlay, InsertOptions, Submit};
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

/// Copies the text to the clipboard and tells the user, for when delivery itself broke down.
fn rescue(app: &AppHandle, text: &str) {
    let _ = platform::copy_text(text);
    status::report(app, "Something went wrong while inserting, so the text was copied to the clipboard. Paste it with Ctrl+V.");
}

/// Cleans a finished dictation, runs the rules on it and sends it where it goes: the preview, the clipboard or the focused app (`deliver`). Inputs: the app handle, the recognised text and the length and window of that dictation. Returns nothing.
/// Silence, noise or only filler sounds are answered with a "Didn't catch any speech" pill (not when the next dictation already owns the overlay), so the user can tell this from a failure.
fn insert(app: &AppHandle, text: String, spoken: Spoken) {
    let began = std::time::Instant::now();
    let prefs = Prefs::load(app);
    let text = clean::clean(&text, &prefs.vocabulary, prefs.remove_fillers);
    if text.is_empty() {
        if is_recording() {
            return;
        }
        return overlay::notice("Didn't catch any speech", prefs.overlay_top, prefs.theme());
    }
    crate::stats::record(app, text.split_whitespace().count() as u32, spoken.ms as f32 / 1000.0);
    let routed = rules::run(Trigger::Dictation, &text);
    let (original, text, output, via) = (text, routed.text, routed.output, routed.matched.join(", "));
    crate::log::note(&format!("dictation: cleaned and routed in {} ms", began.elapsed().as_millis()));
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

/// Sends the text where it belongs. Inputs: the app handle, the text, whether to copy instead of insert (a rule's output or the Output Action setting), the preferences and the window dictation started in. Returns nothing.
/// The text goes to the clipboard when copying is asked for or when focus has moved to another window since dictation started, so it never lands in the wrong app and is never lost. Otherwise it is inserted into the focused app on its own thread, because insertion can wait a few seconds for modifier keys to be released and must not hold up the speech thread; focus is checked again at the moment of delivery, and if the thread cannot start the text is copied.
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
    let options = InsertOptions {
        always_paste: prefs.always_paste,
        restore_clipboard: prefs.restore_clipboard,
        target,
        keep_copy: prefs.keep_copy,
        separator: " ",
        submit: submit_of(&prefs.auto_submit),
    };
    let (handle, context, backup) = (app.clone(), prefs.context_paste, text.clone());
    let spawned = crate::guard::spawn("insert", 512 * 1024, move || {
        insert_text(&handle, text, &options, context);
    });
    if spawned.is_err() {
        let _ = platform::copy_text(&backup);
        status::report(app, "Could not type the text, so it is on your clipboard. Paste it with Ctrl+V.");
    }
}

/// The Auto Submit setting as the key to press.
fn submit_of(setting: &str) -> Submit {
    match setting {
        "Enter" => Submit::Enter,
        "Ctrl+Enter" => Submit::CtrlEnter,
        _ => Submit::None,
    }
}

/// Runs on the insert thread. `context` is Match Surrounding Text: Context-Aware Paste reads the few characters before the cursor and fits the text to them. Insert and Copy and Auto Submit are part of `options`
/// and run inside the platform delivery, after the text really went in; both are off by default.
fn insert_text(app: &AppHandle, text: String, options: &InsertOptions, context: bool) {
    let text = match context.then(platform::text_before_cursor).flatten() {
        Some(before) => self::context::fit(&text, &before),
        None => text,
    };
    let inserting = std::time::Instant::now();
    let inserted = platform::insert(&text, options);
    crate::log::note(&format!("dictation: inserted in {} ms", inserting.elapsed().as_millis()));
    if let Err(e) = inserted {
        status::report_as(app, &e.to_string(), e.pill());
    }
}
