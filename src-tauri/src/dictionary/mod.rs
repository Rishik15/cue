//! Purpose: The dictionary shortcut: look up the selected word in an installed pack and show its meanings in a native card beside the text, with an arrow pointing at it.
//! Contents: on_press / cancel — the shortcut starts a lookup or closes the card (`phase` decides), Escape drops a lookup or closes it; Settle — returns the phase to idle when a lookup ends without a card; is_open — a lookup or card is up; run — copy the selection, look it up, hand the card to `platform::card`; closed — the card went away.
//! The card is drawn natively (no webview), so it appears within tens of milliseconds and costs nothing at rest. Packs are opened read-only for one lookup (`pack`); every lookup is recorded in `words` (`saved` has the Looked Up page's commands).

pub mod catalog;
mod lookup;
mod pack;
mod phase;
pub mod saved;
mod words;

use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::AppHandle;

use crate::platform::{self, CardRequest};
use crate::prefs::Prefs;
use lookup::{lookup, record};
use phase::{press_action, Action, IDLE, LOOKING, OPEN};

/// What the card is doing (see `phase`). Escape and a later press close it, whichever way it is going.
static PHASE: AtomicU8 = AtomicU8::new(IDLE);
/// When the press that started the current lookup happened.
static STARTED: Mutex<Option<Instant>> = Mutex::new(None);
/// Bumped by every press and every cancel; a lookup that finds it changed shows nothing.
static CURRENT: AtomicU64 = AtomicU64::new(0);

/// A lookup is running: the other shortcuts wait for it, or they would copy from the wrong place. A card that is up does not hold them back: they close it.
pub fn is_looking() -> bool {
    PHASE.load(Ordering::Acquire) == LOOKING
}

/// The shortcut starts a lookup; pressing it again (after a short debounce) or Escape closes the card or drops the lookup.
pub fn on_press(app: &AppHandle) {
    let since = STARTED.lock().unwrap_or_else(|e| e.into_inner()).map_or(Duration::MAX, |t| t.elapsed());
    match press_action(PHASE.load(Ordering::Acquire), since) {
        Action::Ignore => {}
        Action::Close => cancel(),
        Action::Start => start(app),
    }
}

/// Returns the phase to idle when lookup `id` ends without a card (nothing to show, or a panic). A lookup that was cancelled leaves the phase alone: `cancel` already reset it, and a newer lookup may own it now.
struct Settle(u64);

impl Drop for Settle {
    fn drop(&mut self) {
        if CURRENT.load(Ordering::Acquire) == self.0 {
            let _ = PHASE.compare_exchange(LOOKING, IDLE, Ordering::AcqRel, Ordering::Acquire);
        }
    }
}

fn start(app: &AppHandle) {
    // Dictation owns the overlay and the keyboard while it records; an open palette has the focus, so copying would read from it.
    if crate::busy::busy(app) {
        return;
    }
    // The time is set under the lock that `on_press` reads it with, before the phase changes hands: a second press right behind this one must see this press's time, not the last lookup's.
    let mut started = STARTED.lock().unwrap_or_else(|e| e.into_inner());
    if PHASE.compare_exchange(IDLE, LOOKING, Ordering::AcqRel, Ordering::Acquire).is_err() {
        return;
    }
    *started = Some(Instant::now());
    drop(started);
    let id = CURRENT.fetch_add(1, Ordering::AcqRel) + 1;
    let app = app.clone();
    // Own thread: copying waits for the app and for the shortcut's keys to be released, which must not hold up the hotkey thread.
    let spawned = crate::guard::spawn("define", 512 * 1024, move || {
        let _settle = Settle(id);
        run(&app, id);
    });
    if let Err(e) = spawned {
        PHASE.store(IDLE, Ordering::Release);
        eprintln!("cue: could not start the dictionary flow: {e}");
    }
}

/// Escape or a later press: drop a lookup in flight and close the card. Runs on the hotkey thread for every Escape, so an idle card costs one atomic read.
pub fn cancel() {
    if PHASE.load(Ordering::Acquire) == IDLE {
        return;
    }
    CURRENT.fetch_add(1, Ordering::AcqRel);
    // A lookup in flight is dropped here (it finds `CURRENT` changed and shows nothing), so nothing else resets its phase.
    if PHASE.compare_exchange(LOOKING, IDLE, Ordering::AcqRel, Ordering::Acquire).is_ok() {
        return;
    }
    if !platform::card::hide() {
        let _ = PHASE.compare_exchange(OPEN, IDLE, Ordering::AcqRel, Ordering::Acquire);
        // no card thread to close anything: nothing may stay marked as up
    }
}

/// A key was typed in the window the word is in: the text moved on, so the card goes (the hotkey thread calls this for keys that are not part of a Cue shortcut).
pub fn dismiss_on_typing() {
    if PHASE.load(Ordering::Acquire) == OPEN {
        cancel();
    }
}

/// The card went away (a press, Escape, a click elsewhere, another window in front): the next press starts a lookup again.
fn closed() {
    let _ = PHASE.compare_exchange(OPEN, IDLE, Ordering::AcqRel, Ordering::Acquire);
    crate::mem::trim_soon(); // the card's bitmap and fonts are freed
}

fn run(app: &AppHandle, id: u64) {
    let begun = Instant::now();
    let Some(pending) = lookup(app, &|| CURRENT.load(Ordering::Acquire) == id) else { return };
    if CURRENT.load(Ordering::Acquire) != id {
        return crate::log::note("dictionary: cancelled before the card was shown");
    }
    let (target, work) = pending.anchor;
    let theme = Prefs::load(app).theme();
    // Only a lookup nobody cancelled in the meantime may open the card (Escape can come between the check above and here).
    if CURRENT.load(Ordering::Acquire) != id || PHASE.compare_exchange(LOOKING, OPEN, Ordering::AcqRel, Ordering::Acquire).is_err() {
        return;
    }
    platform::card::show(CardRequest { sections: pending.sections(), target, work, theme, source: pending.source, on_closed: closed });
    // A cancel that came between the check and the show found no card to hide yet: close it now.
    if CURRENT.load(Ordering::Acquire) != id {
        platform::card::hide();
    }
    record(app, &pending);
    let since = STARTED.lock().unwrap_or_else(|e| e.into_inner()).map_or(0, |t| t.elapsed().as_millis());
    crate::log::note(&format!("dictionary: card shown {since} ms after the press (lookup {} ms)", begun.elapsed().as_millis()));
}
