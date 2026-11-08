//! File: deliver.rs
//! Purpose: Delivers a finished dictation: cleaning and rules on their own thread, then typing or pasting at the cursor, or the clipboard when that cannot be done. The text is never dropped: when it can be neither inserted nor copied it waits in the session recovery slot (`recovery`).
//! Contents: insert_off_thread — the entry from the speech thread (with the length and window of that dictation); insert — clean, run the rules, route (preview, copy or insert); deliver — where the text goes; insert_text — the insert thread (one platform delivery that checks the target window, fits the text to the field it lands in, types or pastes, copies and presses the submit key); copy_or_keep — copy, or keep the text for the session when the copy fails; submit_of — the setting as a key.

use tauri::AppHandle;

use super::{clean, context, is_recording, recovery, Spoken};
use crate::platform::{self, overlay, InsertError, InsertOptions, Submit};
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

/// Copies the text to the clipboard and tells the user, for when delivery itself broke down. A copy that fails keeps the text for the session.
fn rescue(app: &AppHandle, text: &str) {
    match copy_or_keep(text) {
        Ok(()) => {
            status::report(app, "Something went wrong while inserting, so the text was copied to the clipboard. Paste it with Ctrl+V.")
        }
        Err(e) => status::report(app, &format!("Something went wrong while inserting, and {e}")),
    }
}

/// Copies `text` to the clipboard. When that fails the text is kept in the recovery slot and the error says so (it also says when an earlier unrecovered text was replaced). Input: the dictated text. Returns Ok when copied, else the sentence to show after "and".
fn copy_or_keep(text: &str) -> Result<(), String> {
    match platform::copy_text(text) {
        Ok(()) => {
            recovery::clear();
            Ok(())
        }
        Err(e) => {
            let replaced = recovery::hold(text);
            let earlier = if replaced { " An earlier dictated text that was not copied has been replaced." } else { "" };
            Err(format!("the text could not be copied ({e}). Cue is keeping it for this session.{earlier}"))
        }
    }
}

/// Cleans a finished dictation, runs the rules on it and sends it where it goes: the preview, the clipboard or the focused app (`deliver`). Inputs: the app handle, the recognised text and the length and window of that dictation. Returns nothing.
/// Silence, noise or only filler sounds are answered with a "Didn't catch any speech" pill (not when the next dictation already owns the overlay), so the user can tell this from a failure.
fn insert(app: &AppHandle, text: String, spoken: Spoken) {
    let began = std::time::Instant::now();
    let prefs = Prefs::load();
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
/// The text goes to the clipboard when copying is asked for or when focus has moved to another window since dictation started, so it never lands in the wrong app and is never lost (a failed copy keeps it for the session). Otherwise it is inserted into the focused app on its own thread, because insertion can wait a few seconds for modifier keys to be released and must not hold up the speech thread; focus is checked again at the moment of delivery, and if the thread cannot start the text is copied.
fn deliver(app: &AppHandle, text: String, copy: bool, prefs: &Prefs, target: isize) {
    if !on_target(target) {
        match copy_or_keep(&text) {
            Ok(()) => status::report(app, "You switched windows, so the text was copied to the clipboard. Paste it with Ctrl+V."),
            Err(e) => status::report(app, &format!("You switched windows and {e}")),
        }
        return;
    }
    if copy {
        match copy_or_keep(&text) {
            Ok(()) => overlay::notice("Copied to clipboard", prefs.overlay_top, prefs.theme()),
            Err(e) => status::report(app, &format!("Could not copy: {e}")),
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
        fit: prefs.context_paste.then_some(context::fit as fn(&str, &str) -> String),
        ..Default::default()
    };
    let (handle, backup) = (app.clone(), text.clone());
    let spawned = crate::guard::spawn("insert", 512 * 1024, move || {
        insert_text(&handle, text, &options);
    });
    if spawned.is_err() {
        match copy_or_keep(&backup) {
            Ok(()) => status::report(app, "Could not type the text, so it is on your clipboard. Paste it with Ctrl+V."),
            Err(e) => status::report(app, &format!("Could not type the text, and {e}")),
        }
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

/// Runs on the insert thread. Match Surrounding Text (`options.fit`) fits the text to the few characters before the cursor, read by the delivery itself under its lock against the field the text lands in. Insert and Copy and Auto Submit are part of `options` and run inside the platform delivery, after the text really went in; both are off by default.
/// A delivery that did not put the text in and could not copy it either keeps it in the recovery slot; one that went in clears the slot.
fn insert_text(app: &AppHandle, text: String, options: &InsertOptions) {
    let inserting = std::time::Instant::now();
    let inserted = platform::insert(&text, options);
    crate::log::note(&format!("dictation: inserted in {} ms", inserting.elapsed().as_millis()));
    match inserted {
        Ok(()) => recovery::clear(),
        Err(e) => report_failure(app, &text, &e),
    }
}

/// Tells the user how a delivery ended; text that went nowhere and is not on the clipboard is kept for the session and the sentence says so. Inputs: the app handle, the dictated text and the error.
fn report_failure(app: &AppHandle, text: &str, e: &InsertError) {
    if e.text_went_in() {
        recovery::clear();
        return status::report_as(app, &e.to_string(), e.pill());
    }
    if e.copied() {
        recovery::clear();
        return status::report_as(app, &e.to_string(), e.pill());
    }
    let replaced = recovery::hold(text);
    let note = if replaced { " An earlier dictated text that was not copied has been replaced." } else { "" };
    status::report_as(app, &format!("{e} Cue is keeping the text for this session.{note}"), e.pill());
}
