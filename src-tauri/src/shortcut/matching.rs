//! Purpose: Per-event state machines for the hotkey thread: matching (hold-to-talk) and recording a new combo.
//! Contents: step_match — turns key events into press/release of each shortcut and hands them to the actions thread, and Escape cancels what Cue is doing; release_pressed — lets go of every shortcut held down; Live — one reported key event
//! for the settings window; commit — validate and save the combination the window captured (or cancel on Escape), emitting the result.

use handy_keys::{Hotkey, Key, KeyEvent, Modifiers};
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter};

use super::actions::Actions;
use super::keys::{usable, view, Part};
use super::{label, persist, sync_blocking, Binding, Engine, N};

#[derive(Serialize, Clone)]
pub(super) struct Done {
    name: &'static str,
    status: &'static str, // "ok" | "cancelled"
    parts: Vec<Part>,
}

impl Done {
    pub(super) fn cancelled(name: &'static str, parts: Vec<Part>) -> Self {
        Done { name, status: "cancelled", parts }
    }
}

/// One key event during a recording, exactly as the hook saw it (as Handy reports them): the page keeps the display and decides when the combination is complete.
#[derive(Serialize, Clone)]
pub(super) struct Live {
    pub(super) name: &'static str,
    /// This recording's number; the page commits with it, so a key that arrives before `shortcut_record` has returned still commits (no "press twice").
    pub(super) serial: u64,
    pub(super) parts: Vec<Part>,
    pub(super) is_key_down: bool,
    /// A main key took part (not a modifier alone).
    pub(super) keyed: bool,
    /// The combination to commit, serialized; None when nothing is held (every modifier released).
    pub(super) hotkey: Option<Value>,
}

#[derive(Serialize, Clone)]
struct Invalid {
    name: &'static str,
    message: String,
}

/// Matching mode: each shortcut's press and release go to its feature (dictation also emits `dictation-shortcut` for the settings window).
pub fn step_match(actions: &Actions, bindings: &[Binding], ev: &KeyEvent, pressed: &mut [bool; N]) {
    // Escape is never blocked, so the app in front still sees it; Cue just stops what it was doing. Modifiers do not matter: in Hold mode
    // the dictation keys are still down when Escape is pressed. When nothing is being done this is a few atomic reads.
    if ev.is_key_down && ev.key == Some(Key::Escape) {
        crate::dictation::cancel();
        crate::selection::cancel();
        crate::dictionary::cancel();
    }
    // Typing in the window a dictionary card points at moves the text on: the card goes. Keys of Cue's own shortcuts do not count.
    if ev.is_key_down && ev.key.is_some_and(|k| bindings.iter().all(|b| b.active().is_none_or(|h| h.key != Some(k)))) {
        crate::dictionary::dismiss_on_typing();
    }
    for (b, was_down) in bindings.iter().zip(pressed.iter_mut()) {
        // A shortcut the user turned off, or a feature turned off, matches nothing; let go of it first if it was somehow still counted down.
        let Some(h) = b.active() else {
            if std::mem::take(was_down) {
                dispatch(actions, b.name, false);
            }
            continue;
        };
        let down = match (h.key, ev.key) {
            (Some(k), Some(ek)) if k == ek => {
                ev.is_key_down && (h.modifiers.matches(ev.modifiers) || *was_down && still_held(&h, ev.modifiers))
            }
            (Some(_), _) => *was_down && still_held(&h, ev.modifiers),
            (None, None) => h.modifiers.matches(ev.modifiers) || *was_down && still_held(&h, ev.modifiers),
            (None, Some(_)) => *was_down,
        };
        if down != *was_down {
            *was_down = down;
            dispatch(actions, b.name, down);
        }
    }
}

/// Once a shortcut is down it stays down for as long as its own keys are held: touching another modifier meanwhile (Shift, or Alt on the
/// way to the Selection shortcut) must not read as letting go. Starting still needs the exact modifiers, so Ctrl+Shift+Space is not Ctrl+Space.
fn still_held(h: &Hotkey, held: Modifiers) -> bool {
    let wanted = GROUPS.into_iter().filter(|g| h.modifiers.intersects(*g)).fold(Modifiers::empty(), |all, g| all | g);
    h.modifiers.matches(held & wanted)
}

const GROUPS: [Modifiers; 5] = [Modifiers::CMD, Modifiers::SHIFT, Modifiers::CTRL, Modifiers::OPT, Modifiers::FN];

/// True when one shortcut is modifier-only and its modifiers are all among the other's: pressing the longer combo passes through the shorter one, which would fire too.
pub(super) fn overlaps(a: &Hotkey, b: &Hotkey) -> bool {
    let within = |small: &Hotkey, big: &Hotkey| {
        small.key.is_none() && GROUPS.into_iter().filter(|g| small.modifiers.intersects(*g)).all(|g| big.modifiers.intersects(g))
    };
    within(a, b) || within(b, a)
}

/// Sends the release of every shortcut counted as down (a bug, or recording starting, would otherwise leave it recording forever).
pub fn release_pressed(actions: &Actions, bindings: &[Binding], pressed: &mut [bool; N]) {
    for (b, was_down) in bindings.iter().zip(pressed.iter_mut()) {
        if std::mem::take(was_down) {
            dispatch(actions, b.name, false);
        }
    }
}

/// Hands a press or release to the actions thread; the hotkeys thread never waits for a feature to start.
fn dispatch(actions: &Actions, name: &'static str, down: bool) {
    let _ = actions.send((name, down));
}

/// What `commit` decided, acted on after `Shared` is released: saving to disk and emitting must not hold the lock the hook thread needs.
enum Outcome {
    Cancelled(Vec<Part>),
    Invalid(String),
    Saved,
}

/// The settings window captured a combination (`shortcut_commit`): validate it against the other shortcuts and save it, or cancel on Escape. On a clash the recording stays open so the user can press another.
pub fn commit(app: &AppHandle, engine: &Engine, name: &'static str, serial: u64, h: Hotkey) {
    let outcome = {
        let mut s = engine.lock();
        if s.serial != serial || s.recording != Some(name) {
            return; // this recording was cancelled or replaced while the key was in flight
        }
        let current = s.binding(name).map(|b| b.hotkey).unwrap_or(h);
        if h.key == Some(Key::Escape) && h.modifiers.is_empty() {
            s.recording = None;
            sync_blocking(&engine.blocking, &s);
            Outcome::Cancelled(view(&current))
        } else if !usable(&h) {
            Outcome::Invalid(
                "Add a modifier key (Ctrl, Alt, Shift, Win) or use an F-key. A modifier alone needs two, or Alt or Win.".into(),
            )
        } else if let Some(other) = s.bindings.iter().find(|b| b.name != name && b.hotkey == h) {
            Outcome::Invalid(format!("Already used by {}.", label(other.name)))
        } else if let Some(other) = s.bindings.iter().find(|b| b.name != name && overlaps(&b.hotkey, &h)) {
            Outcome::Invalid(format!("Overlaps {}.", label(other.name)))
        } else {
            if let Ok(b) = s.binding(name) {
                b.hotkey = h;
            }
            s.recording = None;
            sync_blocking(&engine.blocking, &s);
            Outcome::Saved
        }
    };
    match outcome {
        Outcome::Cancelled(parts) => {
            let _ = app.emit("shortcut-done", Done { name, status: "cancelled", parts });
        }
        Outcome::Invalid(message) => {
            crate::log::note(&format!("record {name}: not accepted, {message}"));
            let _ = app.emit("shortcut-invalid", Invalid { name, message });
        }
        Outcome::Saved => {
            persist(app, name, &h);
            let _ = app.emit("shortcut-done", Done { name, status: "ok", parts: view(&h) });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extra_modifiers_do_not_release_a_held_shortcut() {
        let ctrl_space = Hotkey::new(Modifiers::CTRL_LEFT, Key::Space).unwrap();
        assert!(still_held(&ctrl_space, Modifiers::CTRL_LEFT));
        assert!(still_held(&ctrl_space, Modifiers::CTRL_LEFT | Modifiers::OPT_LEFT));
        assert!(!still_held(&ctrl_space, Modifiers::OPT_LEFT)); // its own modifier went up
        assert!(!still_held(&ctrl_space, Modifiers::CTRL_RIGHT)); // the left one is required
    }

    #[test]
    fn a_modifier_only_shortcut_overlaps_longer_combos_containing_it() {
        let hk = |m, k: Option<Key>| Hotkey::new(m, k).unwrap();
        let ctrl_alt = hk(Modifiers::CTRL | Modifiers::OPT, None);
        let ctrl_alt_space = hk(Modifiers::CTRL | Modifiers::OPT, Some(Key::Space));
        assert!(overlaps(&ctrl_alt, &ctrl_alt_space) && overlaps(&ctrl_alt_space, &ctrl_alt));
        assert!(!overlaps(&ctrl_alt, &hk(Modifiers::CTRL | Modifiers::SHIFT, None)));
        assert!(!overlaps(&hk(Modifiers::CTRL, Some(Key::Space)), &ctrl_alt_space));
        // neither is modifier-only
    }
}
