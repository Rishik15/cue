//! Purpose: Global shortcuts on handy-keys: one low-level hook serves matching and recording for every named shortcut.
//! Contents: Binding / Shared / Engine — the shortcuts and shared state; init — hook thread; sync_blocking / persist — keep the blocked set and
//! the store in step; cancel_recording — stop a recording whose window closed; get_shortcut / shortcut_record / reset_shortcut — commands taking the shortcut's name. Logic lives in `keys` and `matching`.

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

pub const DICTATION: &str = "dictation";
pub const SELECTION: &str = "selection";
pub const GRAB: &str = "grab";
pub const SNIPPETS: &str = "snippets";
pub const DEFINE: &str = "define";
const NAMES: [&str; 5] = [DICTATION, SELECTION, GRAB, SNIPPETS, DEFINE];

#[derive(Clone, Copy)]
struct Binding {
    name: &'static str,
    hotkey: Hotkey,
}

struct Shared {
    /// A fixed array, so the hook thread copies it for every key event without allocating.
    bindings: [Binding; NAMES.len()],
    /// The shortcut being recorded, if any.
    recording: Option<&'static str>,
}

impl Shared {
    fn binding(&mut self, name: &str) -> Result<&mut Binding, String> {
        self.bindings.iter_mut().find(|b| b.name == name).ok_or_else(|| format!("Unknown shortcut: {name}"))
    }
}

pub struct Engine {
    shared: Arc<Mutex<Shared>>,
    blocking: BlockingHotkeys,
}

/// While recording nothing is blocked, so an old shortcut cannot fire or swallow keys.
fn sync_blocking(blocking: &BlockingHotkeys, s: &Shared) {
    let mut set = blocking.lock().unwrap();
    set.clear();
    if s.recording.is_none() {
        set.extend(s.bindings.iter().map(|b| b.hotkey));
    }
}

fn store_key(name: &str) -> String {
    format!("{name}_hotkey")
}

fn persist(app: &AppHandle, name: &str, h: &Hotkey) {
    if let (Ok(store), Ok(v)) = (app.store(STORE), serde_json::to_value(h)) {
        store.set(store_key(name), v);
        if let Err(e) = store.save() {
            crate::status::report(app, &format!("Could not save the shortcut: {e}. It works until Cue restarts."));
        }
    }
}

fn load(app: &AppHandle, name: &str) -> Hotkey {
    app.store(STORE)
        .ok()
        .and_then(|s| s.get(store_key(name)))
        .and_then(|v| serde_json::from_value::<Hotkey>(v).ok())
        .unwrap_or_else(|| default_hotkey(name))
}

pub fn init(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let blocking: BlockingHotkeys = Arc::new(Mutex::new(HashSet::new()));
    let bindings = NAMES.map(|name| Binding { name, hotkey: load(app, name) });
    let shared = Arc::new(Mutex::new(Shared { bindings, recording: None }));
    sync_blocking(&blocking, &shared.lock().unwrap());
    let listener = KeyboardListener::new_with_blocking(blocking.clone())?;
    let engine = Engine { shared: shared.clone(), blocking: blocking.clone() };
    app.manage(Engine { shared, blocking });

    let app = app.clone();
    // Blocks on `recv`; the vendored hook thread blocks too (vendor/handy-keys/CUE_PATCH.md), so idle CPU is zero.
    crate::guard::spawn("hotkeys", 256 * 1024, move || {
        let (mut pressed, mut rec) = ([false; NAMES.len()], None::<Rec>);
        while let Ok(ev) = listener.recv() {
            // A bug in handling one key event must not end the thread: shortcuts would stay dead until a restart.
            let handled = crate::guard::contain(|| {
                let (bindings, recording) = {
                    let s = engine.shared.lock().unwrap_or_else(|e| e.into_inner());
                    (s.bindings, s.recording)
                };
                if let Some(name) = recording {
                    pressed = [false; NAMES.len()];
                    step_record(&app, &engine, name, rec.get_or_insert_with(Rec::default), &ev);
                } else {
                    rec = None;
                    step_match(&app, &bindings, &ev, &mut pressed);
                }
            });
            if handled.is_none() {
                pressed = [false; NAMES.len()];
            }
        }
        crate::status::report(&app, "Cue's keyboard shortcuts stopped working. Restart Cue to get them back.");
    })?;
    Ok(())
}

/// The window that was recording a shortcut is gone: stop recording, or every shortcut would stay switched off.
pub fn cancel_recording(app: &AppHandle) {
    let Some(engine) = app.try_state::<Engine>() else { return };
    let mut s = engine.shared.lock().unwrap();
    if s.recording.take().is_some() {
        sync_blocking(&engine.blocking, &s);
    }
}

/// The static name for a shortcut named by the UI.
fn known(name: &str) -> Result<&'static str, String> {
    NAMES.iter().copied().find(|n| *n == name).ok_or_else(|| format!("Unknown shortcut: {name}"))
}

#[tauri::command]
pub fn get_shortcut(engine: State<Engine>, name: String) -> Result<Vec<Part>, String> {
    Ok(view(&engine.shared.lock().unwrap().binding(&name)?.hotkey))
}

/// Start (true) or cancel (false) recording; the engine thread finishes it and emits `shortcut-done`.
#[tauri::command]
pub fn shortcut_record(engine: State<Engine>, name: String, start: bool) -> Result<(), String> {
    let mut s = engine.shared.lock().unwrap();
    s.recording = if start { Some(known(&name)?) } else { None };
    sync_blocking(&engine.blocking, &s);
    Ok(())
}

#[tauri::command]
pub fn reset_shortcut(app: AppHandle, engine: State<Engine>, name: String) -> Result<Vec<Part>, String> {
    let mut s = engine.shared.lock().unwrap();
    s.recording = None;
    let hotkey = default_hotkey(&name);
    s.binding(&name)?.hotkey = hotkey;
    sync_blocking(&engine.blocking, &s);
    persist(&app, &name, &hotkey);
    Ok(view(&hotkey))
}
