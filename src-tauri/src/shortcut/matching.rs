//! Purpose: Per-event state machines for the hotkey thread: matching (hold-to-talk) and recording a new combo.
//! Contents: step_match — emits press/release of the active hotkey, and Escape cancels dictation; Rec + step_record — builds a combo from
//! everything held and finishes when all keys are released.

use handy_keys::{Hotkey, Key, KeyEvent, Modifiers};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use super::keys::{usable, view, Part};
use super::{persist, sync_blocking, Engine};

#[derive(Serialize, Clone)]
struct Done {
    status: &'static str, // "ok" | "cancelled"
    parts: Vec<Part>,
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

/// Matching mode: calls the dictation flow and emits `dictation-shortcut` on press/release of the active hotkey.
pub fn step_match(app: &AppHandle, h: &Hotkey, ev: &KeyEvent, pressed: &mut bool) {
    // Escape is never blocked, so the app in front still sees it; Cue just stops what it was doing. Modifiers do not matter: in Hold mode
    // the dictation keys are still down when Escape is pressed. When nothing is being dictated this is one atomic read.
    if ev.is_key_down && ev.key == Some(Key::Escape) {
        crate::dictation::cancel();
    }
    let down = match (h.key, ev.key) {
        (Some(k), Some(ek)) if k == ek => ev.is_key_down && h.modifiers.matches(ev.modifiers),
        (Some(_), _) => *pressed && h.modifiers.matches(ev.modifiers),
        (None, None) => h.modifiers.matches(ev.modifiers),
        (None, Some(_)) => *pressed,
    };
    if down != *pressed {
        *pressed = down;
        let _ = app.emit("dictation-shortcut", down); // lets the open settings window show the held state
        crate::dictation::on_hotkey(app, down);
    }
}

/// Recording mode: accumulates the combo while keys are down, finishes when everything is released.
pub fn step_record(app: &AppHandle, engine: &Engine, rec: &mut Rec, ev: &KeyEvent) {
    if ev.is_key_down {
        rec.mods |= ev.modifiers;
        if let Some(k) = ev.key {
            rec.key = Some(k);
            rec.key_down = true;
        }
        if let Some(h) = rec.hotkey() {
            let _ = app.emit("shortcut-live", view(&h));
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
    finish(app, engine, h);
}

fn finish(app: &AppHandle, engine: &Engine, h: Hotkey) {
    let mut s = engine.shared.lock().unwrap();
    if h.key == Some(Key::Escape) && h.modifiers.is_empty() {
        s.recording = false;
        sync_blocking(&engine.blocking, &s);
        let _ = app.emit("shortcut-done", Done { status: "cancelled", parts: view(&s.active) });
    } else if usable(&h) {
        s.active = h;
        s.recording = false;
        sync_blocking(&engine.blocking, &s);
        persist(app, &h);
        let _ = app.emit("shortcut-done", Done { status: "ok", parts: view(&h) });
    } else {
        let _ = app.emit("shortcut-invalid", "Add a modifier key (Ctrl, Alt, Shift, Win) or use an F-key.");
    }
}
