//! Purpose: Global shortcuts on handy-keys: one low-level hook matches every named shortcut, and a recording listens through a hook of its own (`recorder`).
//! Contents: Binding / Shared / Engine — the shortcuts and shared state (one `Arc<Engine>`; locks recover from poisoning); init — hook thread, lifts held shortcuts
//! on a bug or a switch to recording; sync_blocking / persist — keep the blocked set and the store in step (saved after `Shared` is released); cancel_recording — stop a recording whose window closed; get_shortcut / shortcut_record / reset_shortcut — commands taking the shortcut's name. Logic lives in `keys` and `matching`; `actions` runs what a shortcut does, `recorder` handles one recording.

mod actions;
mod keys;
mod matching;
mod recorder;
mod registry;

use std::collections::HashSet;
use std::sync::{Arc, Mutex, MutexGuard};

use handy_keys::{BlockingHotkeys, Hotkey, KeyboardListener};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_store::StoreExt;

use crate::settings::STORE;
use keys::{view, Part};
use matching::{release_pressed, step_match};
pub(crate) use registry::default_hotkey;
use registry::{label, N, SHORTCUTS};

pub const DICTATION: &str = "dictation";
pub const SELECTION: &str = "selection";
pub const GRAB: &str = "grab";
pub const DEFINE: &str = "define";
pub const NOTES: &str = "notes";

#[derive(Clone, Copy)]
struct Binding {
    name: &'static str,
    /// The combination; always set (reset restores the default). It is kept when turned off, so the chip still shows it, greyed.
    hotkey: Hotkey,
    /// Whether the feature is on (its page's top-bar toggle).
    enabled: bool,
    /// Whether this shortcut itself is on (its own button). Both must be on for the hotkey to fire; either off keeps the hotkey but frees the keys.
    shortcut_on: bool,
}

impl Binding {
    /// The combination that actually fires: the hotkey, unless the feature or the shortcut is turned off.
    fn active(&self) -> Option<Hotkey> {
        (self.enabled && self.shortcut_on).then_some(self.hotkey)
    }
}

/// Whether a feature is turned on (its page's toggle); on by default. Grab has no feature toggle, so it is always on there.
fn feature_enabled(app: &AppHandle, name: &str) -> bool {
    crate::settings::get_bool(app, &format!("{name}_enabled"), true)
}

/// Whether this shortcut itself is on (its own button on the Shortcuts page); on by default.
fn shortcut_on(app: &AppHandle, name: &str) -> bool {
    crate::settings::get_bool(app, &format!("{name}_shortcut_on"), true)
}

struct Shared {
    /// A fixed array, so the hook thread copies it for every key event without allocating.
    bindings: [Binding; N],
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
        // A modifier-only binding would block that modifier in every app; it is still matched, just not swallowed. A shortcut turned off, or a feature turned off, blocks nothing.
        set.extend(s.bindings.iter().filter_map(|b| b.active()).filter(|h| h.key.is_some()));
    }
}

fn store_key(name: &str) -> String {
    format!("{name}_hotkey")
}

fn persist(app: &AppHandle, name: &str, hotkey: &Hotkey) {
    if let (Ok(store), Ok(v)) = (app.store(STORE), serde_json::to_value(hotkey)) {
        store.set(store_key(name), v);
        if let Err(e) = store.save() {
            crate::status::report(app, &format!("Could not save the shortcut: {e}. It works until Cue restarts."));
        }
    }
}

/// The saved combination, or the default when it was never set (or the stored value is unreadable, including an old `null` from a cleared shortcut).
fn load(app: &AppHandle, name: &str) -> Hotkey {
    app.store(STORE)
        .ok()
        .and_then(|s| s.get(store_key(name)))
        .and_then(|v| serde_json::from_value::<Hotkey>(v).ok())
        .unwrap_or_else(|| default_hotkey(name))
}

pub fn init(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let blocking: BlockingHotkeys = Arc::new(Mutex::new(HashSet::new()));
    let bindings = std::array::from_fn(|i| {
        let name = SHORTCUTS[i].name;
        Binding { name, hotkey: load(app, name), enabled: feature_enabled(app, name), shortcut_on: shortcut_on(app, name) }
    });
    let engine = Arc::new(Engine { shared: Mutex::new(Shared { bindings, recording: None, serial: 0 }), blocking: blocking.clone() });
    sync_blocking(&blocking, &engine.lock());
    // Managed before the hook starts: if the hook cannot start, the settings commands still work.
    app.manage(engine.clone());
    let listener = KeyboardListener::new_with_blocking(blocking)?;
    crate::log::note("hotkeys: keyboard hook installed");

    let app = app.clone();
    let actions = actions::start(&app)?;
    // Blocks on `recv`; the vendored hook thread blocks too (vendor/handy-keys/CUE_PATCH.md), so idle CPU is zero.
    crate::guard::spawn("hotkeys", 256 * 1024, move || {
        let mut pressed = [false; N];
        while let Ok(ev) = listener.recv() {
            let (bindings, recording) = {
                let s = engine.lock();
                (s.bindings, s.recording)
            };
            // A bug in handling one key event must not end the thread: shortcuts would stay dead until a restart.
            let handled = crate::guard::contain(|| match recording {
                // A recording has its own hook and thread (`recorder`); matching just lets go of what was held.
                Some(_) => release_pressed(&actions, &bindings, &mut pressed),
                None => step_match(&actions, &bindings, &ev, &mut pressed),
            });
            if handled.is_none() {
                release_pressed(&actions, &bindings, &mut pressed); // the state is unknown after a bug: let go of everything
            }
        }
        crate::status::report(&app, "Cue's keyboard shortcuts stopped working. Restart Cue to get them back.");
    })?;
    Ok(())
}

/// A feature's or a shortcut's on/off changed: re-read every flag so a turned-off one goes inert (and a turned-on one fires again), keeping the stored shortcuts.
pub fn refresh_enabled(app: &AppHandle) {
    let Some(engine) = app.try_state::<Arc<Engine>>() else { return };
    let mut s = engine.lock();
    for b in s.bindings.iter_mut() {
        b.enabled = feature_enabled(app, b.name);
        b.shortcut_on = shortcut_on(app, b.name);
    }
    sync_blocking(&engine.blocking, &s);
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
    registry::spec(name).map(|s| s.name).ok_or_else(|| format!("Unknown shortcut: {name}"))
}

/// The keys to show for a shortcut. Always the stored combination; the page greys it when the shortcut is turned off.
#[tauri::command]
pub fn get_shortcut(engine: State<Arc<Engine>>, name: String) -> Result<Vec<Part>, String> {
    Ok(view(&engine.lock().binding(&name)?.hotkey))
}

/// Start (true) or cancel (false) recording; while it runs the recorder thread reports keys as `shortcut-live` and the window commits with `shortcut_commit`.
/// Returns the number of this recording. A cancel that names an older number is ignored: it belongs to a recording already replaced.
/// A recording listens through a hook of its own, installed now and removed when it ends (as Handy does): the error comes back when Windows refuses it.
#[tauri::command]
pub fn shortcut_record(app: AppHandle, engine: State<Arc<Engine>>, name: String, start: bool, serial: Option<u64>) -> Result<u64, String> {
    let name = known(&name)?;
    let listener = if start { Some(crate::platform::recording_listener()?) } else { None };
    let mut s = engine.lock();
    if !start && serial.is_some_and(|n| n != s.serial) {
        crate::log::note(&format!("shortcut {name}: stale cancel ignored"));
        return Ok(s.serial);
    }
    if start {
        s.serial += 1;
    }
    s.recording = start.then_some(name);
    sync_blocking(&engine.blocking, &s);
    let serial = s.serial;
    drop(s);
    if let Some(listener) = listener {
        recorder::spawn(app, engine.inner().clone(), name, serial, listener)?;
    }
    crate::log::note(&format!("shortcut {name}: recording {}", if start { "started" } else { "cancelled" }));
    Ok(serial)
}

/// The settings window captured a combination and asks Cue to save it (or cancel on Escape); the result comes back as `shortcut-done` or `shortcut-invalid`.
/// `hotkey` is the serialized combination the window read from `shortcut-live`. A `serial` from an older recording is ignored.
#[tauri::command]
pub fn shortcut_commit(
    app: AppHandle,
    engine: State<Arc<Engine>>,
    name: String,
    serial: u64,
    hotkey: serde_json::Value,
) -> Result<(), String> {
    let name = known(&name)?;
    let h: Hotkey = serde_json::from_value(hotkey).map_err(|e| format!("Bad shortcut: {e}"))?;
    matching::commit(&app, engine.inner(), name, serial, h);
    Ok(())
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
