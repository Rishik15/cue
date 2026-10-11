//! Purpose: Per-event state machines for the hotkey thread: matching (hold-to-talk) and recording a new combo.
//! Contents: step_match — routes press/release of each shortcut to its feature, and Escape cancels what Cue is doing; Rec + step_record —
//! builds a combo from everything held and finishes when all keys are released.

use handy_keys::{Hotkey, Key, KeyEvent, Modifiers};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use super::keys::{usable, view, Part};
use super::{persist, sync_blocking, Binding, Engine, DICTATION, NAMES};

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
    }
    for (b, was_down) in bindings.iter().zip(pressed.iter_mut()) {
        let h = &b.hotkey;
        let down = match (h.key, ev.key) {
            (Some(k), Some(ek)) if k == ek => ev.is_key_down && h.modifiers.matches(ev.modifiers),
            (Some(_), _) => *was_down && h.modifiers.matches(ev.modifiers),
            (None, None) => h.modifiers.matches(ev.modifiers),
            (None, Some(_)) => *was_down,
        };
        if down != *was_down {
            *was_down = down;
            dispatch(app, b.name, down);
        }
    }
}

fn dispatch(app: &AppHandle, name: &str, down: bool) {
    if name == DICTATION {
        let _ = app.emit("dictation-shortcut", down); // lets the open settings window show the held state
        crate::dictation::on_hotkey(app, down);
    } else if down {
        crate::selection::on_press(app);
    }
}

/// Recording mode: accumulates the combo while keys are down, finishes when everything is released.
pub fn step_record(app: &AppHandle, engine: &Engine, name: &'static str, rec: &mut Rec, ev: &KeyEvent) {
    if ev.is_key_down {
        rec.mods |= ev.modifiers;
        if let Some(k) = ev.key {
            rec.key = Some(k);
            rec.key_down = true;
        }
        if let Some(h) = rec.hotkey() {
            let _ = app.emit("shortcut-live", Live { name, parts: view(&h) });
        }
        return;
    }
    if ev.key.is_some() {
        rec.key_down = false;
    }
    if !ev.modifiers.is_empty() || rec.key_down {
        return; // something still held
    }
    let Some(h) = rec.hotkey() else { return };
    *rec = Rec::default();
    finish(app, engine, name, h);
}

fn finish(app: &AppHandle, engine: &Engine, name: &'static str, h: Hotkey) {
    let mut s = engine.shared.lock().unwrap();
    let current = s.binding(name).map(|b| b.hotkey).unwrap_or(h);
    let invalid = |message: &str| {
        let _ = app.emit("shortcut-invalid", Invalid { name, message: message.into() });
    };
    if h.key == Some(Key::Escape) && h.modifiers.is_empty() {
        s.recording = None;
        sync_blocking(&engine.blocking, &s);
        let _ = app.emit("shortcut-done", Done { name, status: "cancelled", parts: view(&current) });
    } else if !usable(&h) {
        invalid("Add a modifier key (Ctrl, Alt, Shift, Win) or use an F-key.");
    } else if s.bindings.iter().any(|b| b.name != name && b.hotkey == h) {
        invalid("That combination is already used by another Cue shortcut.");
    } else {
        if let Ok(b) = s.binding(name) {
            b.hotkey = h;
        }
        s.recording = None;
        sync_blocking(&engine.blocking, &s);
        persist(app, name, &h);
        let _ = app.emit("shortcut-done", Done { name, status: "ok", parts: view(&h) });
    }
}
