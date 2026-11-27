//! File: dictionary/mod.rs
//! Purpose: The dictionary shortcut: with a word selected it opens a card beside the word (a webview window with an arrow at the text), with nothing selected it opens a search window next to the pointer. One window and one page serve both; the window exists only while shown.
//! Contents: on_press / start / cancel / escape — the shortcut starts a lookup or closes the window (`State::press` decides), Escape drops a lookup or closes a card; is_looking / card_up / dismiss_on_typing — what the other shortcuts and the keyboard hook ask; installed_book — the packs as one book; owns — whether a window label is the dictionary's.
//! `flow` runs one lookup, `open` publishes its card or search window, `close` is every way a window ends (`dismiss`, `on_focus`, `forget`). `book` answers the two questions (a word's entry, the words that start with a prefix), `placement` is the pure geometry, `state` the session and handshake, `window` the webview window, `commands` what the page calls, `lookup` the selection read. The window is never built ahead and never kept: idle costs nothing. Packs are opened read-only (`pack`): an entry per question, the search of an open window on one connection kept until the window closes or a pack changes (`catalog` holds the gate between reads and a pack's swap or delete); every card lookup and every opened entry is recorded in `words` (`saved` has the Looked Up page's commands).

pub mod book;
pub mod catalog;
mod close;
pub mod commands;
mod copied;
mod decide;
mod flow;
mod label;
mod lookup;
mod open;
mod pack;
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

/// Serializes every test that touches the production `window::BOOK` or runs `publish_pack` / `changed` (which drop it); hold the guard from setup to the last assertion. Return: the guard.
#[cfg(test)]
pub(crate) fn serial_test() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    lock(&LOCK)
}

use crate::lock::lock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use tauri::AppHandle;

use book::Book;
use state::{Phase, Press, State};

pub use close::{dismiss, forget, on_focus};
use close::{dismiss_if, left};
use flow::note_front;
use open::send;

/// The lifecycle: which lookup owns the window, the session and the page's readiness. Every change of hands happens under this lock, so a cancel, a close and a publish are never interleaved.
static STATE: Mutex<State> = Mutex::new(State::CLOSED);
/// The app, for the hook thread's closer, which is a plain function.
static APP: OnceLock<AppHandle> = OnceLock::new();

/// Whether a Card is up, mirrored from `STATE` on every release of its guard. The keyboard hook thread reads it through `card_up`, so the hook never takes the lock that publication holds across disk and event work.
static CARD_UP: AtomicBool = AtomicBool::new(false);

/// The locked state; on drop it refreshes `CARD_UP` before the lock is released, so every mutation reaches the mirror.
pub(crate) struct StateGuard(std::sync::MutexGuard<'static, State>);

impl std::ops::Deref for StateGuard {
    type Target = State;

    /// The locked state, read-only. Return: the state.
    fn deref(&self) -> &State {
        &self.0
    }
}

impl std::ops::DerefMut for StateGuard {
    /// The locked state, mutable. Return: the state.
    fn deref_mut(&mut self) -> &mut State {
        &mut self.0
    }
}

impl Drop for StateGuard {
    /// Publishes whether a Card is up while the lock is still held.
    fn drop(&mut self) {
        CARD_UP.store(self.0.is_card(), Ordering::Release);
    }
}

/// Locks the state, surviving a panic elsewhere. Return: the guard.
fn state() -> StateGuard {
    StateGuard(lock(&STATE))
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
    state().phase() == Phase::Looking
}

/// The shortcut starts a lookup; pressing it again (after a short debounce) closes the window or drops the lookup.
pub fn on_press(app: &AppHandle) {
    let action = state().press(Instant::now());
    match action {
        Press::Ignore => crate::log::note("dictionary: press ignored, within the debounce of the one that started the lookup"),
        Press::Close => cancel(),
        Press::Start => start(app),
    }
}

/// Starts a lookup on its own thread, unless one is up or dictation or a palette has the keyboard (dictation owns the overlay and the keyboard while it records; an open palette has the focus, so copying would read from it). The thread is its own because copying waits for the app and for the shortcut's keys to be released, which must not hold up the hotkey thread. Input: the app. Return: none.
fn start(app: &AppHandle) {
    if crate::busy::busy(app) {
        return crate::log::note("dictionary: press ignored, another feature is busy");
    }
    let _ = APP.set(app.clone());
    let Some(id) = state().begin(Instant::now()) else { return crate::log::note("dictionary: press ignored, a lookup is already up") };
    let app_for_run = app.clone();
    let spawned = crate::guard::spawn("define", 512 * 1024, move || {
        let _settle = flow::Settle(id, app_for_run.clone());
        flow::run(&app_for_run, id);
    });
    if let Err(e) = spawned {
        state().close_if(id, |_| true);
        crate::log::note(&format!("dictionary: could not start the flow: {e}"));
        crate::status::report(app, "Could not look the word up. Try again.");
    }
}

/// A later press, another shortcut or a dictation: drop a lookup in flight and close the window. Runs on the hotkey thread, so an idle window costs one lock.
pub fn cancel() {
    if state().phase() == Phase::Idle {
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

/// Whether a card is up: a typed key in the word's window closes it and Escape is held for it (the search window has the keyboard, so it needs neither). Read on the keyboard hook thread, so it is one atomic load of `CARD_UP` and never waits for the state lock. Return: the answer.
pub fn card_up() -> bool {
    CARD_UP.load(Ordering::Acquire)
}

/// A key was typed in the window the word is in: the text moved on, so the card goes (the hotkey thread calls this for keys that are not part of a Cue shortcut).
pub fn dismiss_on_typing() {
    if card_up() {
        cancel();
    }
}

/// The installed packs as one book. Input: the app.
pub(crate) fn installed_book(app: &AppHandle) -> Book {
    Book::new(catalog::installed_packs(app).into_iter().filter_map(|p| Some((p.id, p.name, catalog::path(app, p)?))).collect())
}
