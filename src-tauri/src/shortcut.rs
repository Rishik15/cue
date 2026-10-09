//! Purpose: Dictation global shortcut on handy-keys: matching, recording, persistence.
//! Contents: Engine — shared state; spawn thread — one low-level keyboard hook serves both matching
//! (hold-to-talk press/release events) and recording (live keys, side-aware modifiers, modifier-only);
//! view — hotkey to display parts for the UI; get_shortcut / shortcut_record / reset_shortcut — commands.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use handy_keys::{BlockingHotkeys, Hotkey, Key, KeyEvent, KeyboardListener, Modifiers};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_store::StoreExt;

use crate::settings::STORE;

const STORE_KEY: &str = "dictation_hotkey";

/// One displayed key: `code` picks the icon, `side` is 'L'/'R' for side-specific modifiers.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Part {
    pub code: String,
    pub label: String,
    pub side: Option<char>,
}

#[derive(Serialize, Clone)]
struct Done {
    status: &'static str, // "ok" | "cancelled"
    parts: Vec<Part>,
}

struct Shared {
    active: Hotkey,
    recording: bool,
}

pub struct Engine {
    shared: Arc<Mutex<Shared>>,
    blocking: BlockingHotkeys,
}

fn default_hotkey() -> Hotkey {
    Hotkey::new(Modifiers::CTRL_LEFT, Key::Space).expect("valid default")
}

/// Hotkey to display parts. Win+Shift+F23 is what the Copilot key sends, shown as one key.
pub fn view(h: &Hotkey) -> Vec<Part> {
    let m = h.modifiers;
    let part = |code: &str, label: &str, side| Part { code: code.into(), label: label.into(), side };
    if m.intersects(Modifiers::CMD) && m.intersects(Modifiers::SHIFT) && h.key == Some(Key::F23) {
        return vec![part("copilot", "Copilot", None)];
    }
    let mut parts = Vec::new();
    let groups = [
        ("ctrl", "Ctrl", Modifiers::CTRL_LEFT, Modifiers::CTRL_RIGHT),
        ("alt", "Alt", Modifiers::OPT_LEFT, Modifiers::OPT_RIGHT),
        ("shift", "Shift", Modifiers::SHIFT_LEFT, Modifiers::SHIFT_RIGHT),
        ("win", "Win", Modifiers::CMD_LEFT, Modifiers::CMD_RIGHT),
    ];
    for (code, label, l, r) in groups {
        match (m.contains(l), m.contains(r)) {
            (true, false) => parts.push(part(code, label, Some('L'))),
            (false, true) => parts.push(part(code, label, Some('R'))),
            (true, true) => parts.push(part(code, label, None)),
            _ => {}
        }
    }
    if m.contains(Modifiers::FN) {
        parts.push(part("fn", "Fn", None));
    }
    if let Some(k) = h.key {
        let label = k.to_string();
        parts.push(part(&label.to_lowercase(), &label, None));
    }
    parts
}

/// A bare non-F-key would hijack normal typing.
fn usable(h: &Hotkey) -> bool {
    let is_f = h.key.is_some_and(|k| matches!(k.to_string().as_str(), s if s.starts_with('F') && s[1..].chars().all(|c| c.is_ascii_digit()) && s.len() > 1));
    !h.modifiers.is_empty() || is_f
}

/// While recording nothing is blocked, so the old shortcut cannot fire or swallow keys.
fn sync_blocking(blocking: &BlockingHotkeys, s: &Shared) {
    let mut set = blocking.lock().unwrap();
    set.clear();
    if !s.recording {
        set.insert(s.active);
    }
}

fn persist(app: &AppHandle, h: &Hotkey) {
    if let (Ok(store), Ok(v)) = (app.store(STORE), serde_json::to_value(h)) {
        store.set(STORE_KEY, v);
        let _ = store.save();
    }
}

struct Rec {
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

/// Matching mode: emits `dictation-shortcut` true/false on press/release of the active hotkey.
fn step_match(app: &AppHandle, h: &Hotkey, ev: &KeyEvent, pressed: &mut bool) {
    let down = match (h.key, ev.key) {
        (Some(k), Some(ek)) if k == ek => ev.is_key_down && h.modifiers.matches(ev.modifiers),
        (Some(_), _) => *pressed && h.modifiers.matches(ev.modifiers),
        (None, None) => h.modifiers.matches(ev.modifiers),
        (None, Some(_)) => *pressed,
    };
    if down != *pressed {
        *pressed = down;
        let _ = app.emit("dictation-shortcut", down);
    }
}

/// Recording mode: builds the combo from everything held, finishes when all keys are released.
fn step_record(app: &AppHandle, engine: &Engine, rec: &mut Rec, ev: &KeyEvent) {
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

pub fn init(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let active = app
        .store(STORE)
        .ok()
        .and_then(|s| s.get(STORE_KEY))
        .and_then(|v| serde_json::from_value::<Hotkey>(v).ok())
        .unwrap_or_else(default_hotkey);
    let blocking: BlockingHotkeys = Arc::new(Mutex::new(HashSet::new()));
    let shared = Arc::new(Mutex::new(Shared { active, recording: false }));
    sync_blocking(&blocking, &shared.lock().unwrap());
    let listener = KeyboardListener::new_with_blocking(blocking.clone())?;
    let engine = Engine { shared: shared.clone(), blocking: blocking.clone() };
    app.manage(Engine { shared, blocking });

    let app = app.clone();
    // Blocks on `recv` (no polling in our thread). ponytail: handy-keys' own hook thread still wakes about every
    // 100 ms to check its shutdown flag; measure idle wakeups in cue-bench, patch/fork the crate if it breaks the budget.
    std::thread::Builder::new().name("hotkeys".into()).stack_size(256 * 1024).spawn(move || {
        let (mut pressed, mut rec) = (false, None::<Rec>);
        while let Ok(ev) = listener.recv() {
            let (active, recording) = {
                let s = engine.shared.lock().unwrap();
                (s.active, s.recording)
            };
            if recording {
                pressed = false;
                step_record(&app, &engine, rec.get_or_insert_with(Rec::default), &ev);
            } else {
                rec = None;
                step_match(&app, &active, &ev, &mut pressed);
            }
        }
    })?;
    Ok(())
}

#[tauri::command]
pub fn get_shortcut(engine: State<Engine>) -> Vec<Part> {
    view(&engine.shared.lock().unwrap().active)
}

/// Start (true) or cancel (false) recording; the engine thread finishes it and emits `shortcut-done`.
#[tauri::command]
pub fn shortcut_record(engine: State<Engine>, start: bool) {
    let mut s = engine.shared.lock().unwrap();
    s.recording = start;
    sync_blocking(&engine.blocking, &s);
}

#[tauri::command]
pub fn reset_shortcut(app: AppHandle, engine: State<Engine>) -> Vec<Part> {
    let mut s = engine.shared.lock().unwrap();
    s.active = default_hotkey();
    s.recording = false;
    sync_blocking(&engine.blocking, &s);
    persist(&app, &s.active);
    view(&s.active)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codes(h: &str) -> Vec<(String, Option<char>)> {
        view(&h.parse::<Hotkey>().unwrap()).into_iter().map(|p| (p.code, p.side)).collect()
    }

    #[test]
    fn view_and_validation() {
        assert_eq!(codes("Ctrl+Space"), [("ctrl".into(), None), ("space".into(), None)]);
        assert_eq!(view(&default_hotkey())[0].side, Some('L'));
        let left = Hotkey::new(Modifiers::CTRL_LEFT, Key::Grave).unwrap();
        assert_eq!(view(&left)[0].side, Some('L'));
        assert_eq!(view(&left)[1].code, "`");
        let copilot = Hotkey::new(Modifiers::CMD_LEFT | Modifiers::SHIFT_LEFT, Key::F23).unwrap();
        assert_eq!(view(&copilot)[0].code, "copilot");
        assert!(usable(&Hotkey::new(Modifiers::empty(), Key::F9).unwrap()));
        assert!(usable(&Hotkey::new(Modifiers::OPT_LEFT, None).unwrap())); // modifier-only
        assert!(!usable(&Hotkey::new(Modifiers::empty(), Key::A).unwrap()));
    }
}
