//! File: input/app.rs
//! Purpose: Starts input inside the app: the settings-backed store, the effects thread that runs shortcut actions and observers off the hook thread, and the Windows pump that owns the hook. Everything here may touch Tauri and settings; the reducer behind it may not.
//! Contents: SettingsStore — bindings and switches in settings.json; ChannelSink — the bounded queue to the effects thread; start — builds and starts everything; refresh_enabled — re-reads the on/off switches for the running input.

use std::sync::atomic::AtomicU32;
use std::sync::mpsc::{self, SyncSender};
use std::sync::Arc;

use tauri::{AppHandle, Emitter, Manager};

use super::api::Input;
use super::catalog::{Catalog, Run};
use super::chord::Chord;
use super::core::Core;
use super::driver::{ArmOutcome, Runtime};
use super::effects::{Effect, Gate, Gens, Sink};
use super::error::InputError;
use super::mailbox::Mailbox;
use super::save::Store;
use super::shared::{install_global, Shared};

const QUEUE: usize = 256;

/// Bindings and the on/off switches in settings.json, under the names the page and the features already use.
struct SettingsStore;

impl Store for SettingsStore {
    /// The stored combination; an old `null` from a cleared shortcut counts as never set.
    fn load(&self, id: &str) -> Option<serde_json::Value> {
        crate::settings::get(&format!("{id}_hotkey")).filter(|v| !v.is_null())
    }

    /// Writes the combination; a read-only or unwritable settings file is a readable error and the value stays unsaved.
    fn save(&self, id: &str, chord: &Chord) -> Result<(), String> {
        let value = serde_json::to_value(chord).map_err(|e| format!("Could not save the shortcut: {e}."))?;
        crate::settings::update(&[(format!("{id}_hotkey").as_str(), Some(value))])
    }

    /// A shortcut fires only when its feature and the shortcut itself are both on.
    fn enabled(&self, id: &str) -> bool {
        crate::settings::get_bool(&format!("{id}_enabled"), true) && crate::settings::get_bool(&format!("{id}_shortcut_on"), true)
    }
}

/// The queue from the input thread to the effects thread. `send` never blocks; a full queue returns false, the input module fails open, and a release that was refused is owed and still runs (`Gens::owe`).
struct ChannelSink(SyncSender<Effect>);

impl Sink for ChannelSink {
    /// Queues the effect.
    fn send(&mut self, effect: Effect) -> bool {
        self.0.try_send(effect).is_ok()
    }

    /// Queues a no-op so the effects thread looks at owed releases; a full queue needs no wake, the thread is busy.
    fn wake(&mut self) {
        let _ = self.0.try_send(Effect::Check);
    }
}

/// Carries out one effect on the effects thread.
fn carry_out(app: &AppHandle, catalog: &'static Catalog, gate: &mut Gate, effect: Effect) {
    match effect {
        Effect::Action { idx, down, gen } => {
            if !gate.admit(idx as usize, down, gen) {
                return;
            }
            let spec = &catalog.specs[idx as usize];
            match spec.run {
                Run::Hold(run) => run(app, down),
                Run::Press(run) if down => run(app),
                Run::Press(_) => {}
            }
        }
        Effect::Check => {}
        Effect::Report(message) => crate::status::report(app, &message),
        Effect::Escape => catalog.on_escape.iter().for_each(|f| f()),
        Effect::Typing => catalog.on_key_down.iter().for_each(|f| f()),
        Effect::MenuMask => crate::platform::send_menu_mask(),
        Effect::Recorder(hub, view) => {
            let _ = app.emit_to("settings", "shortcut-recorder", &view);
            hub.publish(view);
        }
        Effect::Catalog(hub, view) => {
            let _ = app.emit_to("settings", "shortcut-catalog", &view);
            hub.publish(view);
        }
    }
}

/// Runs the release of a Hold shortcut whose release could not be queued. Input: the app, the catalog and the shortcut's index.
fn release_owed(app: &AppHandle, catalog: &'static Catalog, idx: usize) {
    if let Run::Hold(run) = catalog.specs[idx].run {
        run(app, false);
    }
}

/// Starts the effects thread and returns the queue to it.
fn spawn_effects(app: &AppHandle, catalog: &'static Catalog, gens: Arc<Gens>) -> Result<Box<dyn Sink>, InputError> {
    let (tx, rx) = mpsc::sync_channel::<Effect>(QUEUE);
    let app = app.clone();
    crate::guard::spawn("cue-input-effects", 256 * 1024, move || {
        let mut gate = Gate::new(gens, catalog.specs.len());
        while let Ok(effect) = rx.recv() {
            let _ = crate::guard::contain(|| carry_out(&app, catalog, &mut gate, effect));
            for idx in gate.owed_releases() {
                let _ = crate::guard::contain(|| release_owed(&app, catalog, idx));
            }
        }
    })
    .map_err(|e| InputError::Unavailable(format!("Cue could not start its shortcuts: {e}")))?;
    Ok(Box::new(ChannelSink(tx)))
}

/// Starts input: loads and validates the stored shortcuts, starts the effects thread and installs the keyboard hook. Returns when the hook is installed, or the reason it could not be.
pub fn start(app: &AppHandle, catalog: &'static Catalog) -> Result<Input, InputError> {
    let thread = Arc::new(AtomicU32::new(0));
    let waker = thread.clone();
    let mailbox = Arc::new(Mailbox::new(move || crate::platform::wake_input(waker.load(std::sync::atomic::Ordering::Acquire))));
    let shared = Arc::new(Shared::default());
    install_global(shared.clone());
    let core = Core::new(catalog, Arc::new(SettingsStore), mailbox.clone(), shared.clone());
    let (hub, gens) = (core.catalog_hub(), core.gens());
    let sink = spawn_effects(app, catalog, gens)?;
    let runtime = Runtime::new(core, sink, mailbox.clone(), || ArmOutcome::Later);
    let pump = crate::platform::start_input_pump(thread, runtime).map_err(InputError::Unavailable)?;
    crate::log::note("input: keyboard hook installed");
    Ok(Input::new(mailbox, hub, catalog, Box::new(move || pump.stop())))
}

/// A feature's or a shortcut's on/off changed: tells the running input to re-read every switch.
pub fn refresh_enabled(app: &AppHandle) {
    if let Some(input) = app.try_state::<Input>() {
        input.refresh_enabled();
    }
}
