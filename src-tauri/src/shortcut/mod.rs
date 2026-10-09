//! Purpose: Dictation global shortcut on handy-keys: one low-level hook serves matching and recording.
//! Contents: Engine — shared state; init — hook thread; sync_blocking / persist — keep the blocked set and
//! the store in step; get_shortcut / shortcut_record / reset_shortcut — commands. Logic lives in `keys` and `matching`.

mod keys;
mod matching;

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use handy_keys::{BlockingHotkeys, Hotkey, KeyboardListener};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_store::StoreExt;

use crate::settings::STORE;
use keys::{default_hotkey, view, Part};
use matching::{step_match, step_record, Rec};

const STORE_KEY: &str = "dictation_hotkey";

struct Shared {
    active: Hotkey,
    recording: bool,
}

pub struct Engine {
    shared: Arc<Mutex<Shared>>,
    blocking: BlockingHotkeys,
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

fn load_active(app: &AppHandle) -> Hotkey {
    app.store(STORE)
        .ok()
        .and_then(|s| s.get(STORE_KEY))
        .and_then(|v| serde_json::from_value::<Hotkey>(v).ok())
        .unwrap_or_else(default_hotkey)
}

pub fn init(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let blocking: BlockingHotkeys = Arc::new(Mutex::new(HashSet::new()));
    let shared = Arc::new(Mutex::new(Shared { active: load_active(app), recording: false }));
    sync_blocking(&blocking, &shared.lock().unwrap());
    let listener = KeyboardListener::new_with_blocking(blocking.clone())?;
    let engine = Engine { shared: shared.clone(), blocking: blocking.clone() };
    app.manage(Engine { shared, blocking });

    let app = app.clone();
    // Blocks on `recv`; the vendored hook thread blocks too (vendor/handy-keys/CUE_PATCH.md), so idle CPU is zero.
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
