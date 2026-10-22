//! Purpose: Per-event state machines for the hotkey thread: matching (hold-to-talk) and recording a new combo.
//! Contents: step_match — routes press/release of each shortcut to its feature, and Escape cancels what Cue is doing; release_pressed — lets go of every shortcut held down; Rec + step_record —
//! builds a combo from what is held and finishes when its key comes up (a modifier-only combo, when the last modifier is up).

use handy_keys::{Hotkey, Key, KeyEvent, Modifiers};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use super::keys::{usable, view, Part};
use super::{persist, sync_blocking, Binding, Engine, DEFINE, DICTATION, GRAB, NAMES, NOTES, SELECTION};

#[derive(Serialize, Clone)]
struct Done {
    name: &'static str,
    status: &'static str, // "ok" | "cancelled"
    parts: Vec<Part>,
}

#[derive(Serialize, Clone)]
struct Live {
    name: &'static str,
    parts: Vec<Part>,
}

#[derive(Serialize, Clone)]
struct Invalid {
    name: &'static str,
    message: String,
}

pub struct Rec {
    mods: Modifiers,
    key: Option<Key>,
    key_down: bool,
}

impl Default for Rec {
    fn default() -> Self {
        Rec { mods: Modifiers::empty(), key: None, key_down: false }
    }
}

impl Rec {
    fn hotkey(&self) -> Option<Hotkey> {
        Hotkey::new(self.mods, self.key).ok()
    }
}

/// Matching mode: each shortcut's press and release go to its feature (dictation also emits `dictation-shortcut` for the settings window).
pub fn step_match(app: &AppHandle, bindings: &[Binding], ev: &KeyEvent, pressed: &mut [bool; NAMES.len()]) {
    // Escape is never blocked, so the app in front still sees it; Cue just stops what it was doing. Modifiers do not matter: in Hold mode
    // the dictation keys are still down when Escape is pressed. When nothing is being done this is a few atomic reads.
    if ev.is_key_down && ev.key == Some(Key::Escape) {
        crate::dictation::cancel();
        crate::selection::cancel();
        crate::dictionary::cancel();
    }
    // Typing in the window a dictionary card points at moves the text on: the card goes. Keys of Cue's own shortcuts do not count.
    if ev.is_key_down && ev.key.is_some_and(|k| bindings.iter().all(|b| b.hotkey.key != Some(k))) {
        crate::dictionary::dismiss_on_typing();
    }
    for (b, was_down) in bindings.iter().zip(pressed.iter_mut()) {
        let h = &b.hotkey;
        let down = match (h.key, ev.key) {
            (Some(k), Some(ek)) if k == ek => {
                ev.is_key_down && (h.modifiers.matches(ev.modifiers) || *was_down && still_held(h, ev.modifiers))
            }
            (Some(_), _) => *was_down && still_held(h, ev.modifiers),
            (None, None) => h.modifiers.matches(ev.modifiers) || *was_down && still_held(h, ev.modifiers),
            (None, Some(_)) => *was_down,
        };
        if down != *was_down {
            *was_down = down;
            dispatch(app, b.name, down);
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
pub fn release_pressed(app: &AppHandle, bindings: &[Binding], pressed: &mut [bool; NAMES.len()]) {
    for (b, was_down) in bindings.iter().zip(pressed.iter_mut()) {
        if std::mem::take(was_down) {
            dispatch(app, b.name, false);
        }
    }
}

fn dispatch(app: &AppHandle, name: &str, down: bool) {
    if name == DICTATION {
        let _ = app.emit("dictation-shortcut", down); // lets the open settings window show the held state
        return crate::dictation::on_hotkey(app, down);
    }
    if !down {
        return;
    }
    match name {
        SELECTION => crate::selection::on_press(app),
        GRAB => crate::grab::on_press(app),
        DEFINE => crate::dictionary::on_press(app),
        NOTES => crate::notes::on_press(app),
        _ => {}
    }
}

/// Recording mode: accumulates the combo while keys are down, finishes when its key comes up (a modifier-only combo, when the last modifier does).
pub fn step_record(app: &AppHandle, engine: &Engine, name: &'static str, rec: &mut Rec, ev: &KeyEvent) {
    // Only while a shortcut is being recorded (a few keys, no typed text): what the hook delivered tells a dead hook from a wrong combination on another machine.
    let key = ev.key.map_or_else(|| "-".to_string(), |k| k.to_string());
    crate::log::note(&format!("record {name}: {} {key} with {:?}", if ev.is_key_down { "down" } else { "up" }, ev.modifiers));
    if ev.is_key_down {
        // The combination is what is held when the key goes down; a modifier added after the key is not part of it.
        if let Some(k) = ev.key {
            if !(rec.key == Some(k) && rec.key_down) {
                rec.mods = ev.modifiers; // not on auto-repeat: letting go of a modifier while the key is held must not change the combination
            }
            rec.key = Some(k);
            rec.key_down = true;
        } else if rec.key.is_none() {
            rec.mods |= ev.modifiers;
        }
        if let Some(h) = rec.hotkey() {
            let _ = app.emit("shortcut-live", Live { name, parts: view(&h) });
        }
        return;
    }
    // A combination with a key is complete when that key comes up, whatever modifiers are still down (Handy does the same): waiting for every modifier made one lost
    // modifier release hang the recording. A modifier-only combination is complete when the last modifier is up.
    match ev.key {
        Some(k) if rec.key == Some(k) => rec.key_down = false,
        Some(_) => return, // another key came up; the recorded one is still down
        None if rec.key.is_some() || !ev.modifiers.is_empty() => return,
        None => {}
    }
    let Some(h) = rec.hotkey() else { return };
    *rec = Rec::default();
    finish(app, engine, name, h);
}

/// What `finish` decided, acted on after `Shared` is released: saving to disk and emitting must not hold the lock the hook thread needs.
enum Outcome {
    Cancelled(Vec<Part>),
    Invalid(&'static str),
    Saved,
}

fn finish(app: &AppHandle, engine: &Engine, name: &'static str, h: Hotkey) {
    let outcome = {
        let mut s = engine.lock();
        let current = s.binding(name).map(|b| b.hotkey).unwrap_or(h);
        if h.key == Some(Key::Escape) && h.modifiers.is_empty() {
            s.recording = None;
            sync_blocking(&engine.blocking, &s);
            Outcome::Cancelled(view(&current))
        } else if !usable(&h) {
            Outcome::Invalid("Add a modifier key (Ctrl, Alt, Shift, Win) or use an F-key. A modifier alone needs two, or Alt or Win.")
        } else if s.bindings.iter().any(|b| b.name != name && b.hotkey == h) {
            Outcome::Invalid("That combination is already used by another Cue shortcut.")
        } else if s.bindings.iter().any(|b| b.name != name && overlaps(&b.hotkey, &h)) {
            Outcome::Invalid("That combination overlaps another Cue shortcut.")
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
            let _ = app.emit("shortcut-invalid", Invalid { name, message: message.into() });
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
