//! File: selection.rs
//! Purpose: The selection shortcut: copy the selected text and open the Transform Text strip beside it (`strip`), a few chips and a command list of every action that changes it; what the user pastes goes back into the field.
//! Contents: on_press — shortcut pressed, run the flow on its own thread (one at a time; a press while the strip is up closes it), logging the press and why one is ignored; gate — what a press does; cancel — Escape drops a flow still running; run — build the strip window, copy the selection, build the rows, open the strip (cancellation is checked before the copy, after it and right before the strip opens); same_selection — whether two readings are one selection; anchor — where the selection is on the screen; notice — log and show a pill line; selected_text_for — the copied selection or why there is none.
//! Escape bumps `CURRENT`, so a cancelled flow opens nothing.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;

use tauri::AppHandle;

use crate::actions::Source;
use crate::platform::{self, overlay, Capture, Rect, Target};
use crate::prefs::Prefs;
use crate::rules::{self, model::Trigger};
use crate::{palette, status, strip, text};

/// Longest selection an action will work on: a runaway select-all in a huge document must not stall the flow or fill memory.
const MAX_SELECTION_CHARS: usize = 200_000;

/// Counts flows and cancellations: a flow whose id is no longer current was cancelled.
static CURRENT: AtomicU64 = AtomicU64::new(0);
/// A flow is running. A second press meanwhile is ignored: two flows would both send Ctrl+C and fight over the clipboard.
static BUSY: AtomicBool = AtomicBool::new(false);

/// A selection is being read or its strip is being built.
pub fn is_busy() -> bool {
    BUSY.load(Ordering::Acquire)
}

/// The selection shortcut was pressed. A card that is up gives way, and a strip that is up closes (a second press is a way out). Dictation owns the overlay and the keyboard while it records, so copying then would put Ctrl+C into the middle of it; an open preview has the focus, so copying would read from it, not from the user's app.
/// The flow runs on its own thread, because copying waits for the app and for the shortcut's keys to be released, which must not hold up the hotkey thread. A flow that panics closes its strip at once, so the next press is not spent closing it.
/// Every press logs `selection: press`, and every press that starts no flow logs one `selection: press ignored: <reason>` line, so a missed press says where it stopped.
pub fn on_press(app: &AppHandle) {
    crate::dictionary::cancel();
    crate::log::note("selection: press");
    match gate(strip::is_open(app), BUSY.load(Ordering::Acquire), crate::busy::busy_with(app)) {
        Gate::Close => {
            ignored("the strip was open, closed it");
            return strip::close(app);
        }
        Gate::Ignore(by) => return ignored(&format!("busy with {by}")),
        Gate::Run => {}
    }
    if BUSY.swap(true, Ordering::AcqRel) {
        return ignored("a selection flow started meanwhile");
    }
    let id = CURRENT.fetch_add(1, Ordering::AcqRel) + 1;
    text::Cancel::ring();
    let handle = app.clone();
    let started = crate::guard::spawn_or_report(app, "selection", 512 * 1024, "The selection flow", move || {
        let _release = crate::guard::Release(&BUSY);
        if let Err(panic) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(&handle, id))) {
            strip::dismiss(&handle);
            std::panic::resume_unwind(panic);
        }
    });
    if !started {
        BUSY.store(false, Ordering::Release);
        ignored("the selection thread could not start");
    }
}

/// What a press does, from the state at the press.
#[derive(Debug, PartialEq, Eq)]
enum Gate {
    /// Close the open strip (a second press is a way out).
    Close,
    /// Do nothing: the named feature already uses the screen or keyboard.
    Ignore(&'static str),
    /// Start a flow.
    Run,
}

/// Decides a press. Input: whether the strip is open, whether a selection flow runs, and the feature that is busy (`busy::busy_with`). Return: the decision; an open strip closes only when no flow is still building it.
fn gate(strip_open: bool, running: bool, busy_with: Option<&'static str>) -> Gate {
    match busy_with {
        _ if strip_open && !running => Gate::Close,
        Some(by) => Gate::Ignore(by),
        None => Gate::Run,
    }
}

/// Logs why a press started no flow. Input: the reason (no user text).
fn ignored(why: &str) {
    crate::log::note(&format!("selection: press ignored: {why}"));
}

/// Escape: drops a flow that is still running.
pub fn cancel() {
    CURRENT.fetch_add(1, Ordering::AcqRel);
    text::Cancel::ring();
}

/// Logs the line and shows it on the pill.
fn notice(text: &str) {
    crate::log::note(&format!("selection: {text}"));
    let prefs = Prefs::load();
    overlay::notice(text, prefs.overlay_top, prefs.theme());
}

/// One flow. The strip window is built first thing, hidden and unfocusable (it must never take the focus, or Ctrl+C would go to it instead of the user's app), so its page loads while the selection is copied; focus is handed back to the app as a second guard. A saved action that uses the text model loads it meanwhile.
fn run(app: &AppHandle, id: u64) {
    let pressed = Instant::now();
    let token = platform::capture_target();
    if !strip::warm(app) {
        return crate::log::note("selection: stopped: the strip window could not be built");
    }
    if platform::foreground_window() != token.window {
        platform::focus_window(token.window);
    }
    if let Some(task) = rules::model_task_for(Trigger::Selection) {
        text::prepare(task, text::Cancel::new(move || CURRENT.load(Ordering::Acquire) != id));
    }
    let cancelled = || CURRENT.load(Ordering::Acquire) != id;
    let Some((text, html, token)) = selected_text_for(app, token, &cancelled).filter(|_| !cancelled()) else { return strip::dismiss(app) };
    let program = platform::foreground_app();
    let items = palette::build(&text, &program, html.as_deref(), Source::Selection);
    if items.is_empty() {
        strip::dismiss(app);
        return notice("No action changes this text");
    }
    let Some(place) = anchor(&text) else {
        strip::dismiss(app);
        return notice("Could not find the selection on the screen");
    };
    if cancelled() {
        return strip::dismiss(app);
    }
    strip::open(app, items, (text, program), token, place, pressed);
}

/// Whether two readings of one selection are the same text: the first 32 characters with all whitespace removed, so a copy and a UI Automation read that differ in line breaks or length still match. Input: the two texts. Return: the answer.
pub(crate) fn same_selection(a: &str, b: &str) -> bool {
    let start = |t: &str| t.split_whitespace().flat_map(str::chars).take(32).collect::<String>();
    start(a) == start(b)
}

/// Where the selection is, with the work area and scale of the one monitor it is on: the rectangle UI Automation gives for the selection when it reads the same text, else the caret, else the pointer. Input: the copied text. Return: None when nothing can be placed.
fn anchor(text: &str) -> Option<(Rect, Rect, f32)> {
    let rect = platform::uia_selection(64).filter(|found| same_selection(&found.text, text)).and_then(|found| found.rect);
    platform::selection_anchor(rect)
}

/// The selected text and its HTML (when the program offered any) for the place named by `token`, or None after saying why there is none; the token is taken before anything slow happened; the third value is the token of the field the text was copied from, which delivery checks.
fn selected_text_for(app: &AppHandle, token: Target, cancelled: &dyn Fn() -> bool) -> Option<(String, Option<String>, Target)> {
    match platform::capture(Target { runtime_id: None, key: String::new(), ..token }, cancelled) {
        Capture::Text(copied) if copied.text.chars().count() > MAX_SELECTION_CHARS => notice("Selection is too long"),
        Capture::Text(copied) => match crate::sanitize::clean_selection(&copied.text) {
            clean if clean.is_empty() => notice("Nothing selected, or Cue cannot see it"),
            clean => {
                if !copied.restored {
                    status::report(app, "Cue could not put your previous clipboard back after reading the selection.");
                }
                return Some((clean, copied.html, copied.target));
            }
        },
        Capture::Empty => notice("Nothing selected, or Cue cannot see it"),
        Capture::Unknown => notice("Cue could not tell what was selected, so it used nothing"),
        Capture::Cancelled => crate::log::note("selection: cancelled before the copy was used"),
        Capture::Refused(why) => status::report(app, &why),
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{gate, Gate};

    /// An open strip with no flow building it closes; a busy feature names itself; an idle app runs; a strip still being built by a running flow is busy, not closed.
    #[test]
    fn a_press_is_gated_with_a_reason() {
        assert_eq!(gate(true, false, Some("strip")), Gate::Close);
        assert_eq!(gate(false, false, Some("dictation")), Gate::Ignore("dictation"));
        assert_eq!(gate(true, true, Some("strip")), Gate::Ignore("strip"));
        assert_eq!(gate(false, true, Some("selection")), Gate::Ignore("selection"));
        assert_eq!(gate(false, false, None), Gate::Run);
    }
}
