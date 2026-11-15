//! File: dictionary/mod.rs
//! Purpose: The dictionary shortcut: with a word selected it opens a card beside the word (a webview window with an arrow at the text), with nothing selected it opens a search window in the middle of the screen. One window and one page serve both; the window exists only while shown.
//! Contents: on_press / cancel / escape — the shortcut starts a lookup or closes the window (`phase` decides), Escape drops a lookup or closes a card; Settle — ends a lookup that shows nothing; is_looking / wants_typing / dismiss_on_typing — what the other shortcuts and the keyboard hook ask; run / proceed — read the selection and open the card or the search window, unless the user moved on; on_focus / forget / dismiss / dismiss_if — the window lost the keyboard, is gone, or is closed (a close names the lookup it is for, so an old one cannot close a new one).
//! `book` answers the two questions (a word's entry, the words that start with a prefix), `placement` is the pure geometry, `state` the session and handshake, `window` the webview window, `commands` what the page calls, `lookup` the selection read. The window is never built ahead and never kept: idle costs nothing. Packs are opened read-only for one question (`pack`); every card lookup and every opened entry is recorded in `words` (`saved` has the Looked Up page's commands).

pub mod book;
pub mod catalog;
pub mod commands;
mod copied;
mod label;
mod lookup;
mod pack;
mod phase;
mod placement;
pub mod saved;
mod state;
mod window;
mod words;

#[cfg(test)]
mod book_tests;
#[cfg(test)]
mod placement_tests;
#[cfg(test)]
mod tests;

use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde_json::json;
use tauri::{AppHandle, Emitter};

use book::Book;
use lookup::{notice, pick, Picked, Selected};
use phase::{press_action, Action, IDLE};
use state::{Anchor, Mode, Session, State};

/// The lifecycle: which lookup owns the window, the session and the page's readiness. Every change of hands happens under this lock, so a cancel, a close and a publish are never interleaved.
static STATE: Mutex<State> = Mutex::new(State::CLOSED);
/// The app, for the hook thread's closer, which is a plain function.
static APP: OnceLock<AppHandle> = OnceLock::new();
/// The open event the page receives.
const OPEN_EVENT: &str = "dict-open";
/// How many recent words the empty search window lists.
const RECENT: usize = 8;

/// Locks the state, surviving a panic elsewhere. Return: the guard.
fn state() -> std::sync::MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

/// Whether a window label belongs to the dictionary (its windows are named for the lookup they serve). Input: the label.
pub fn owns(label: &str) -> bool {
    label::id_of(label).is_some()
}

/// Milliseconds since the press that started the current lookup, for the timing log.
fn since_press() -> u128 {
    state().since_press().map_or(0, |t| t.as_millis())
}

/// A lookup is running: the other shortcuts wait for it, or they would copy from the wrong place. A window that is up does not hold them back: they close it.
pub fn is_looking() -> bool {
    state().phase() == phase::LOOKING
}

/// The shortcut starts a lookup; pressing it again (after a short debounce) closes the window or drops the lookup.
pub fn on_press(app: &AppHandle) {
    let (phase, since) = {
        let s = state();
        (s.phase(), s.since_press().unwrap_or(Duration::MAX))
    };
    match press_action(phase, since) {
        Action::Ignore => {}
        Action::Close => cancel(),
        Action::Start => start(app),
    }
}

/// Ends lookup `id` when it shows nothing (nothing to show, a refusal, a panic, a cancel): the phase returns to idle and the window built for it is destroyed. The window is the lookup's own (its label carries the number), so a newer lookup's window is never touched.
struct Settle(u64, AppHandle);

impl Drop for Settle {
    /// Gives the lookup up and destroys its window unless it opened.
    fn drop(&mut self) {
        let kept = state().settle(self.0);
        if !kept {
            window::destroy(&self.1, self.0);
        }
    }
}

/// Starts a lookup on its own thread, unless one is up or dictation or a palette has the keyboard (dictation owns the overlay and the keyboard while it records; an open palette has the focus, so copying would read from it). The thread is its own because copying waits for the app and for the shortcut's keys to be released, which must not hold up the hotkey thread. Input: the app. Return: none.
fn start(app: &AppHandle) {
    if crate::busy::busy(app) {
        return;
    }
    let _ = APP.set(app.clone());
    let Some(id) = state().begin(Instant::now()) else { return };
    let app_for_run = app.clone();
    let spawned = crate::guard::spawn("define", 512 * 1024, move || {
        let _settle = Settle(id, app_for_run.clone());
        run(&app_for_run, id);
    });
    if let Err(e) = spawned {
        state().close_if(id, |_| true);
        crate::log::note(&format!("dictionary: could not start the flow: {e}"));
        crate::status::report(app, "Could not look the word up. Try again.");
    }
}

/// A later press, another shortcut or a dictation: drop a lookup in flight and close the window. Runs on the hotkey thread, so an idle window costs one lock.
pub fn cancel() {
    if state().phase() == IDLE {
        return;
    }
    if let Some(app) = APP.get() {
        dismiss(app);
    }
}

/// Escape: closes a lookup or a card; the search window handles Escape itself (back to the results, then close), because it has the keyboard.
pub fn escape() {
    if state().is_search() {
        return;
    }
    cancel();
}

/// Whether a typed key matters now: only while a card is up (the search window is typed into), so ordinary typing costs nothing otherwise.
pub fn wants_typing() -> bool {
    state().is_card()
}

/// A key was typed in the window the word is in: the text moved on, so the card goes (the hotkey thread calls this for keys that are not part of a Cue shortcut).
pub fn dismiss_on_typing() {
    if wants_typing() {
        cancel();
    }
}

/// Closes whatever is up at once: the lookup is dropped, the session forgotten and the window destroyed. Input: the app.
pub fn dismiss(app: &AppHandle) {
    let id = state().close();
    window::destroy(app, id);
}

/// Closes lookup or window `id` when it is still the current one and `allowed` says so, in one step, and destroys its window; an older number changes nothing. Input: the app, the number and the condition (on the locked state). Return: whether it was closed.
fn dismiss_if(app: &AppHandle, id: u64, allowed: impl FnOnce(&State) -> bool) -> bool {
    let closed = state().close_if(id, allowed);
    if let Some(id) = closed {
        window::destroy(app, id);
    }
    closed.is_some()
}

/// The user clicked or switched away from card `id` (called on its click-away watch's thread, which names it): closes it when it is still the open card.
fn left(id: u64) {
    if let Some(app) = APP.get() {
        if dismiss_if(app, id, State::is_card) {
            crate::log::note(&format!("dictionary: the card was closed by the user leaving it, {} ms after the press", since_press()));
        }
    }
}

/// Focus moved to or from window `label`: once the search window has held the keyboard, losing it (a click elsewhere) closes it. A card never takes the keyboard, so it never gets here. Input: the app, the window's label and whether it gained focus.
pub fn on_focus(app: &AppHandle, label: &str, focused: bool) {
    let Some(id) = label::id_of(label) else { return };
    if focused {
        return state().focused(id);
    }
    dismiss_if(app, id, |s| s.held_focus(id));
}

/// The window `label` is gone (closed by any means): when it was the current one nothing may stay marked as up, and the next press starts a lookup again. A destroyed event of an older window changes nothing. Input: the app and the label.
pub fn forget(app: &AppHandle, label: &str) {
    if let Some(id) = label::id_of(label) {
        dismiss_if(app, id, |_| true);
    }
}

/// The installed packs as one book. Input: the app.
pub(crate) fn installed_book(app: &AppHandle) -> Book {
    Book::new(catalog::installed_packs(app).into_iter().filter_map(|p| Some((p.id, p.name, catalog::path(app, p)?))).collect())
}

/// One lookup: builds the hidden window, reads the selection and opens the card for a word or the search window for nothing. A lookup nobody cancelled in the meantime, whose window the shortcut was pressed in is still in front, is the only one that opens anything. Input: the app and the lookup's number.
fn run(app: &AppHandle, id: u64) {
    let target = crate::platform::capture_target();
    let source = target.window;
    if !window::warm(app, id) {
        return;
    }
    let wanted = || state().wanted(id);
    let picked = pick(app, &target, &wanted);
    if matches!(picked, Picked::Stop) {
        return;
    }
    match proceed(picked, wanted(), crate::platform::in_front(source)) {
        Some(Picked::Word(selected)) => open_card(app, id, source, &selected),
        Some(_) => open_search(app, id, source),
        None => crate::log::note("dictionary: cancelled, or the word's window left the front, before the window was shown"),
    }
}

/// Whether what a lookup read still opens a window: not when it was cancelled, and not when the window the shortcut was pressed in has left the front meanwhile (a selection read that failed because of the switch looks like nothing selected, and the search window would then take the keyboard from the app the user went to). Input: what was read, whether the lookup is still wanted and whether that window is in front. Return: what to open, None for nothing.
fn proceed(picked: Picked, wanted: bool, in_front: bool) -> Option<Picked> {
    (wanted && in_front).then_some(picked)
}

/// The monitor the lines are on (the one they overlap most, else the nearest) and the lines cut to its work area. Input: the lines. Return: the lines, the work area and the scale, None when Windows lists no monitor.
fn on_display(lines: &[crate::platform::Rect]) -> Option<(Vec<crate::platform::Rect>, crate::platform::Rect, f32)> {
    let displays = crate::platform::displays();
    let works: Vec<_> = displays.iter().map(|d| d.0).collect();
    let (work, scale) = displays[placement::choose_display(lines, &works)?];
    Some((placement::clip_lines(lines, work), work, scale))
}

/// Opens the card for a selected word: the word's entry, the place of the selection, one session. Says why when there is nothing to show. Input: the app, the lookup's number, the word's window and the selection.
fn open_card(app: &AppHandle, id: u64, source: isize, selected: &Selected) {
    let say = |text: &str| {
        if state().wanted(id) {
            notice(text);
        }
    };
    let book = installed_book(app);
    if book.is_empty() {
        return say("Download a dictionary first");
    }
    let entry = match book.lookup(&selected.text) {
        Ok(Some(entry)) => entry,
        Ok(None) if pack::normalize(&selected.text).is_none() => return say("Select one word"),
        Ok(None) => return say(&format!("No definition for {}", pack::normalize(&selected.text).unwrap_or_default())),
        Err(e) => return crate::status::report(app, &e),
    };
    let caret = crate::platform::caret_in(source);
    let Some((lines, kind)) = placement::anchor(&selected.lines, caret, crate::platform::pointer_box()) else {
        return say("Could not find the selection on the screen");
    };
    let Some((lines, work, scale)) = on_display(&lines) else { return say("Could not find the selection on the screen") };
    let anchor = Anchor { lines, kind, rtl: placement::is_rtl(&selected.text) };
    let open = json!({ "session": id, "mode": "card", "theme": crate::settings::get_theme(), "entry": entry });
    let session = Session { id, mode: Mode::Card, anchor: Some(anchor), work, scale, source, placed: None, open: Some(open) };
    if publish(app, id, session) {
        saved::record(app, &entry);
    }
}

/// Opens the search window in the middle of the monitor the user is working on, listing the words looked up lately. Input: the app, the lookup's number and the window the shortcut was pressed in.
fn open_search(app: &AppHandle, id: u64, source: isize) {
    let fallback = || crate::platform::displays().first().copied();
    let Some((work, scale)) = crate::platform::work_area_of(source).or_else(fallback) else {
        return notice("Could not find the screen");
    };
    let pack = !installed_book(app).is_empty();
    let recent = saved::recent(app, RECENT);
    let open = json!({ "session": id, "mode": "search", "theme": crate::settings::get_theme(), "recent": recent, "pack": pack });
    let session = Session { id, mode: Mode::Search, anchor: None, work, scale, source, placed: None, open: Some(open) };
    publish(app, id, session);
}

/// Stores the session and sends the open event once the page is ready; the lookup becomes an open window. A lookup cancelled or replaced meanwhile opens nothing: the check, the change of phase and the store are one step under the lock. Input: the app, the lookup's number and the session (which says its mode). Return: whether the window was opened.
fn publish(app: &AppHandle, id: u64, session: Session) -> bool {
    let mut s = state();
    let Some(ready) = s.publish(id, session) else { return false };
    let at = s.since_press().map_or(0, |t| t.as_millis());
    crate::log::note(&format!("dictionary: window ready at {at} ms"));
    if ready {
        send(app, &mut s);
    }
    true
}

/// Sends the open event the caller claimed to the window it is for; a failed send leaves it to be claimed again. Input: the app and the locked state.
fn send(app: &AppHandle, s: &mut State) {
    let Some((id, payload)) = s.claim() else { return };
    match app.emit_to(label::label(id), OPEN_EVENT, payload) {
        Ok(()) => {
            let at = s.since_press().map_or(0, |t| t.as_millis());
            crate::log::note(&format!("dictionary: open event sent at {at} ms"));
        }
        Err(e) => {
            crate::log::note(&format!("dictionary: the open event failed: {e}"));
            s.unsent();
        }
    }
}
