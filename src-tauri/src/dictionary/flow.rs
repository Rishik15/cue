//! File: dictionary/flow.rs
//! Purpose: One lookup from the press to the window: build the hidden window, read the selection, decide what the selection opens and open it, unless the user moved on.
//! Contents: Settle — ends a lookup that shows nothing; run — the lookup's thread; note_front — logs a foreground check that dropped it; show — opens what `decide` chose; say — a Notice for a lookup still wanted; proceed — whether a read selection still opens a window.

use tauri::AppHandle;

use super::decide::{decide, Decision};
use super::lookup::{notice, pick, Picked};
use super::open::{open_card, open_search};
use super::{installed_book, state, window};

/// Ends lookup `id` when it shows nothing (nothing to show, a refusal, a panic, a cancel): the phase returns to idle and the window built for it is destroyed. The window is the lookup's own (its label carries the number), so a newer lookup's window is never touched.
pub(super) struct Settle(pub(super) u64, pub(super) AppHandle);

impl Drop for Settle {
    /// Gives the lookup up and destroys its window unless it opened.
    fn drop(&mut self) {
        let kept = state().settle(self.0);
        if !kept {
            window::destroy(&self.1, self.0);
        }
    }
}

/// One lookup: builds the hidden window, reads the selection and opens the card for a word or the search window for nothing. A lookup nobody cancelled in the meantime, whose window the shortcut was pressed in is still in front, is the only one that opens anything. Input: the app and the lookup's number.
pub(super) fn run(app: &AppHandle, id: u64) {
    let target = crate::platform::capture_target();
    let source = target.window;
    if !window::warm(app, id) {
        return crate::log::note("dictionary: stopped, the window could not be built");
    }
    let wanted = || state().wanted(id);
    let picked = pick(app, &target, &wanted);
    if matches!(picked, Picked::Stop) {
        return crate::log::note("dictionary: stopped, the selection read was cancelled or refused");
    }
    let still = wanted();
    match proceed(picked, still, crate::platform::in_front(source)) {
        Some(picked) => show(app, id, source, picked),
        None => note_front("before-publish", source, still),
    }
}

/// Logs a lookup dropped at a foreground check: the stage, whether it was still wanted, the captured window with its root, the root in front now, both roots' window classes and whether the front one is Cue's own (handles and classes only, never text). Input: the stage, the word's window and whether the lookup is still wanted.
pub(super) fn note_front(stage: &str, source: isize, wanted: bool) {
    let (root, front) = crate::platform::roots(source);
    let (root_class, _) = crate::platform::class_and_owner(root);
    let (front_class, front_cue) = crate::platform::class_and_owner(front);
    crate::log::note(&format!(
        "dictionary: dropped {stage}, wanted {wanted}, captured {source:#x} root {root:#x} class {root_class:?}, front root {front:#x} class {front_class:?} cue {front_cue}"
    ));
}

/// Shows what `decide` chose for what was read: the Card, the Search window, a Notice or a reported failure. Input: the app, the lookup's number, the word's window and what was read.
fn show(app: &AppHandle, id: u64, source: isize, picked: Picked) {
    let selected = match picked {
        Picked::Word(selected) => Some(selected),
        Picked::Nothing | Picked::Stop => None,
    };
    let decision = decide(selected, &installed_book(app));
    let name = match &decision {
        Decision::Card(..) => "card",
        Decision::Search => "search",
        Decision::Notice(_) => "notice",
        Decision::Failed(_) => "failed",
    };
    crate::log::note(&format!("dictionary: decided {name}"));
    match decision {
        Decision::Card(selected, entry) => open_card(app, id, source, &selected, entry),
        Decision::Search => open_search(app, id, source),
        Decision::Notice(text) => say(id, &text),
        Decision::Failed(why) => crate::status::report(app, &why),
    }
}

/// Shows a Notice for lookup `id` while it is still wanted. Input: the number and the sentence.
pub(super) fn say(id: u64, text: &str) {
    if state().wanted(id) {
        notice(text);
    }
}

/// Whether what a lookup read still opens a window: not when it was cancelled, and not when the window the shortcut was pressed in has left the front meanwhile (a selection read that failed because of the switch looks like nothing selected, and the search window would then take the keyboard from the app the user went to). Input: what was read, whether the lookup is still wanted and whether that window is in front. Return: what to open, None for nothing.
pub(super) fn proceed(picked: Picked, wanted: bool, in_front: bool) -> Option<Picked> {
    (wanted && in_front).then_some(picked)
}
