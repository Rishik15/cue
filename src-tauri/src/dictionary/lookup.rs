//! File: dictionary/lookup.rs
//! Purpose: What the dictionary shortcut finds at the press: the selected text and the line rectangles it occupies on the screen, or the knowledge that nothing is selected (the search window opens then).
//! Contents: Selected — a selection and its lines; Picked — a selection, nothing selected, or a flow that ended; pick — read it, bound to the window the shortcut was pressed in; notice — a short message pill; place_after_copy — the lines of a copied selection once an app's accessibility tree is ready.

use std::time::Duration;

use tauri::AppHandle;

use crate::platform::{self, overlay, Capture, Rect, Target};
use crate::prefs::Prefs;
use crate::status;

/// UI Automation is asked for no more than this. The copy fallback returns whatever is selected, so `pack::normalize` is what refuses a longer selection.
const MAX_SELECTION: usize = 64;

/// A selection as the dictionary sees it.
pub(super) struct Selected {
    pub text: String,
    /// One rectangle per visible line, in physical pixels; empty when the app gave none.
    pub lines: Vec<Rect>,
}

/// What reading the press found.
pub(super) enum Picked {
    Word(Selected),
    /// Nothing is selected, or the app cannot say: the search window opens.
    Nothing,
    /// The flow ended: it was cancelled, or the reason was already shown (a password field, an administrator app).
    Stop,
}

/// Shows a short message on the pill. Input: the sentence; it never carries the user's word into the log.
pub(super) fn notice(text: &str) {
    crate::log::note("dictionary: notice shown");
    let prefs = Prefs::load();
    overlay::notice(text, prefs.overlay_top, prefs.theme());
}

/// The first UI Automation call makes a browser build its accessibility tree, and the tree is not always ready when the copy has finished: ask again for the lines only, a few times, so the first lookup in a browser sits at the word like the later ones (otherwise it falls back to the caret or pointer, a little off). Both texts are compared by the same rule, so lines of another selection are never taken. Input: the copied text and the target. Return: the lines, empty when none came.
fn place_after_copy(text: &str, target: &Target) -> Vec<Rect> {
    const TRIES: u32 = 5;
    for attempt in 0..TRIES {
        let found = platform::uia_selection_for(target, MAX_SELECTION).filter(|found| crate::selection::same_selection(&found.text, text));
        if let Some(found) = found.filter(|f| !f.lines.is_empty()) {
            return found.lines;
        }
        if attempt + 1 < TRIES {
            std::thread::sleep(Duration::from_millis(40));
        }
    }
    Vec::new()
}

/// Reads the selection of the window the shortcut was pressed in: UI Automation first (text and lines at once, without touching the clipboard), only an app that does not expose its text is copied from, still bound to the field captured at the press. Input: the app, the token taken at the press and whether the lookup is still wanted. Return: what was found; a refusal (password field, administrator app) is reported to the user and ends the flow.
pub(super) fn pick(app: &AppHandle, target: &Target, wanted: &dyn Fn() -> bool) -> Picked {
    if let Some(found) = platform::uia_selection_for(target, MAX_SELECTION) {
        return Picked::Word(Selected { text: found.text, lines: found.lines });
    }
    match platform::capture_field(target.clone(), &|| !wanted()) {
        Capture::Text(copied) => {
            let (warning, text) = super::copied::read(&copied);
            if let Some(line) = warning {
                status::report(app, line);
            }
            if text.is_empty() {
                return Picked::Nothing;
            }
            Picked::Word(Selected { lines: place_after_copy(&text, target), text })
        }
        Capture::Empty | Capture::Unknown => Picked::Nothing,
        Capture::Cancelled => Picked::Stop,
        Capture::Refused(why) => {
            status::report(app, &why);
            Picked::Stop
        }
    }
}
