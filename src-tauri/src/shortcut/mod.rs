//! Purpose: Global shortcuts on handy-keys: one low-level hook serves matching and recording for every named shortcut.
//! Contents: Binding / Shared / Engine — the shortcuts and shared state (one `Arc<Engine>`; locks recover from poisoning); init — hook thread, lifts held shortcuts
//! on a bug or a switch to recording; sync_blocking / persist — keep the blocked set and the store in step (saved after `Shared` is released); cancel_recording — stop a recording whose window closed; get_shortcut / shortcut_record / shortcut_hook_check / reset_shortcut — commands taking the shortcut's name. Logic lives in `keys` and `matching`.

mod keys;
mod matching;

use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use handy_keys::{BlockingHotkeys, Hotkey, KeyboardListener};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_store::StoreExt;

use crate::settings::STORE;
use keys::{default_hotkey, view, Part};
use matching::{release_pressed, step_match, step_record, Rec};

pub const DICTATION: &str = "dictation";
pub const SELECTION: &str = "selection";
pub const GRAB: &str = "grab";
pub const DEFINE: &str = "define";
pub const NOTES: &str = "notes";
const NAMES: [&str; 5] = [DICTATION, SELECTION, GRAB, DEFINE, NOTES];

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
    /// Counts recordings started, so a cancel that belongs to an earlier one cannot end a newer one.
    serial: u64,
}

impl Shared {
    fn binding(&mut self, name: &str) -> Result<&mut Binding, String> {
        self.bindings.iter_mut().find(|b| b.name == name).ok_or_else(|| format!("Unknown shortcut: {name}"))
    }
}

pub struct Engine {
    shared: Mutex<Shared>,
    /// Key events the hook delivered since the current recording began; the settings window compares it with the keys it saw itself (`shortcut_hook_check`).
    seen: AtomicU64,
    blocking: BlockingHotkeys,
}

impl Engine {
    /// A panic elsewhere must not leave every shortcut command failing on a poisoned lock.
    fn lock(&self) -> MutexGuard<'_, Shared> {
        self.shared.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// While recording nothing is blocked, so an old shortcut cannot fire or swallow keys.
fn sync_blocking(blocking: &BlockingHotkeys, s: &Shared) {
    let mut set = blocking.lock().unwrap_or_else(|e| e.into_inner());
    set.clear();
    if s.recording.is_none() {
        // A modifier-only binding would block that modifier in every app; it is still matched, just not swallowed.
        set.extend(s.bindings.iter().map(|b| b.hotkey).filter(|h| h.key.is_some()));
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
    let engine = Arc::new(Engine {
        shared: Mutex::new(Shared { bindings, recording: None, serial: 0 }),
        seen: AtomicU64::new(0),
        blocking: blocking.clone(),
    });
    sync_blocking(&blocking, &engine.lock());
    // Managed before the hook starts: if the hook cannot start, the settings commands still work.
    app.manage(engine.clone());
    let listener = KeyboardListener::new_with_blocking(blocking)?;
    crate::log::note("hotkeys: keyboard hook installed");

    let app = app.clone();
    // Blocks on `recv`; the vendored hook thread blocks too (vendor/handy-keys/CUE_PATCH.md), so idle CPU is zero.
    crate::guard::spawn("hotkeys", 256 * 1024, move || {
        let (mut pressed, mut rec) = ([false; NAMES.len()], None::<Rec>);
        while let Ok(ev) = listener.recv() {
            let (bindings, recording) = {
                let s = engine.lock();
                (s.bindings, s.recording)
            };
            // A bug in handling one key event must not end the thread: shortcuts would stay dead until a restart.
            let handled = crate::guard::contain(|| match recording {
                Some(name) => {
                    engine.seen.fetch_add(1, Ordering::Relaxed);
                    release_pressed(&app, &bindings, &mut pressed); // recording takes over: a held dictation key must not stay "down"
                    step_record(&app, &engine, name, rec.get_or_insert_with(Rec::default), &ev);
                }
                None => {
                    rec = None;
                    step_match(&app, &bindings, &ev, &mut pressed);
                }
            });
            if handled.is_none() {
                release_pressed(&app, &bindings, &mut pressed); // the state is unknown after a bug: let go of everything
            }
        }
        crate::status::report(&app, "Cue's keyboard shortcuts stopped working. Restart Cue to get them back.");
    })?;
    Ok(())
}

/// The window that was recording a shortcut is gone: stop recording, or every shortcut would stay switched off.
pub fn cancel_recording(app: &AppHandle) {
    let Some(engine) = app.try_state::<Arc<Engine>>() else { return };
    let mut s = engine.lock();
    if s.recording.take().is_some() {
        sync_blocking(&engine.blocking, &s);
    }
}

/// The static name for a shortcut named by the UI.
fn known(name: &str) -> Result<&'static str, String> {
    NAMES.iter().copied().find(|n| *n == name).ok_or_else(|| format!("Unknown shortcut: {name}"))
}

#[tauri::command]
pub fn get_shortcut(engine: State<Arc<Engine>>, name: String) -> Result<Vec<Part>, String> {
    Ok(view(&engine.lock().binding(&name)?.hotkey))
}

/// Start (true) or cancel (false) recording; the engine thread finishes it and emits `shortcut-done`.
/// Returns the number of this recording. A cancel that names an older number is ignored: it belongs to a recording already replaced.
#[tauri::command]
pub fn shortcut_record(engine: State<Arc<Engine>>, name: String, start: bool, serial: Option<u64>) -> Result<u64, String> {
    let name = known(&name)?;
    let mut s = engine.lock();
    if !start && serial.is_some_and(|n| n != s.serial) {
        crate::log::note(&format!("shortcut {name}: stale cancel ignored"));
        return Ok(s.serial);
    }
    if start {
        s.serial += 1;
        // Windows can drop the long-lived hook without a word; Handy installs a fresh one per recording, so do the same before any key is pressed.
        crate::platform::rearm_keyboard_hook();
    }
    s.recording = start.then_some(name);
    engine.seen.store(0, Ordering::Relaxed);
    crate::log::note(&format!("shortcut {name}: recording {}", if start { "started" } else { "cancelled" }));
    sync_blocking(&engine.blocking, &s);
    Ok(s.serial)
}

/// The settings window saw a key during a recording. False when the hook delivered none: it is dead or deaf here, so it is installed again and the page says so.
#[tauri::command]
pub fn shortcut_hook_check(engine: State<Arc<Engine>>) -> bool {
    if engine.seen.load(Ordering::Relaxed) > 0 {
        return true;
    }
    crate::log::note("record: the window saw keys and the keyboard hook delivered none, installing the hook again");
    crate::platform::rearm_keyboard_hook();
    false
}

#[tauri::command]
pub fn reset_shortcut(app: AppHandle, engine: State<Arc<Engine>>, name: String) -> Result<Vec<Part>, String> {
    let name = known(&name)?;
    let hotkey = default_hotkey(name);
    {
        let mut s = engine.lock();
        if s.bindings.iter().any(|b| b.name != name && b.hotkey == hotkey) {
            return Err("Another Cue shortcut already uses the default combination. Change that one first.".into());
        }
        if s.bindings.iter().any(|b| b.name != name && matching::overlaps(&b.hotkey, &hotkey)) {
            return Err("That combination overlaps another Cue shortcut.".into());
        }
        s.recording = None;
        s.binding(name)?.hotkey = hotkey;
        sync_blocking(&engine.blocking, &s);
    }
    persist(&app, name, &hotkey);
    Ok(view(&hotkey))
}
