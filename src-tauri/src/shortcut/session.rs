//! File: shortcut/session.rs
//! Purpose: Owns who is recording: starts a recording for a window's request id, ends it exactly once with its one final event, saves a finished combination, and answers the settings window's commands. The serial that tells recordings apart lives here and nowhere else.
//! Contents: Host — where events and saves go; begin / cancel_request / end / cancel_any / commit — the recording's lifecycle (a save is written under the persistence lock and undone if the store fails) (every path that ends one emits its `shortcut-done` here); shortcut_record / shortcut_record_cancel / cancel_recording — the commands and the window-closed hook.

use std::sync::Arc;

use handy_keys::Hotkey;
use tauri::{AppHandle, State};

use super::events::{Done, Event, Outcome, Reason, Saved};
use super::keys::view;
use super::{known, matching, persist, recorder, sync_blocking, Engine, Shared};

/// What a recording needs from the app: where its events go and how a saved combination is stored. The app wires the window and the store in; a test wires in memory.
pub(super) struct Host<'a> {
    pub(super) engine: &'a Engine,
    pub(super) emit: &'a dyn Fn(Event),
    pub(super) persist: &'a dyn Fn(&'static str, &Hotkey) -> Result<(), String>,
}

/// The final event for a recording, showing the shortcut as it is in `s` now.
fn done(s: &Shared, name: &'static str, serial: u64, outcome: Outcome) -> Event {
    let parts = s.bindings.iter().find(|b| b.name == name).map(|b| view(&b.hotkey)).unwrap_or_default();
    Event::Done(Done { name, serial, outcome, parts })
}

/// Whether `name`'s recording number `serial` is the one running.
pub(super) fn current(engine: &Engine, name: &'static str, serial: u64) -> bool {
    let s = engine.lock();
    s.serial == serial && s.recording == Some(name)
}

/// Held while a recording arms the shared hook, so a recording that was replaced while waiting never arms after the one that replaced it.
static ARMING: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// How many cancelled ids are remembered for starts that arrive late.
const REMEMBERED_CANCELS: usize = 32;

/// Starts a recording of `name` for the window's `request` id and returns its serial, or `None` when that request was cancelled before it started. A recording already running is ended with `cancelled(replaced)`.
pub(super) fn begin(host: &Host, name: &'static str, request: &str) -> Option<u64> {
    let (serial, replaced) = {
        let mut s = host.engine.lock();
        if let Some(at) = s.cancelled.iter().position(|r| r == request) {
            s.cancelled.remove(at);
            return None;
        }
        let replaced = s.recording.map(|old| done(&s, old, s.serial, Outcome::Cancelled { reason: Reason::Replaced }));
        s.serial += 1;
        s.recording = Some(name);
        s.request = request.to_string();
        sync_blocking(&host.engine.blocking, &s);
        (s.serial, replaced)
    };
    if let Some(event) = replaced {
        (host.emit)(event);
    }
    Some(serial)
}

/// Cancels the recording the window knows as `request`: ends it if it is the running one, otherwise remembers the id so a start that has not run yet does nothing. Returns whether a running recording ended.
pub(super) fn cancel_request(host: &Host, name: &'static str, request: &str) -> bool {
    let serial = {
        let mut s = host.engine.lock();
        if s.recording == Some(name) && s.request == request {
            Some(s.serial)
        } else {
            if s.cancelled.len() == REMEMBERED_CANCELS {
                s.cancelled.remove(0);
            }
            s.cancelled.push(request.to_string());
            None
        }
    };
    serial.is_some_and(|serial| end(host, name, serial, Outcome::Cancelled { reason: Reason::Outside }))
}

/// Ends recording `serial` of `name` with `outcome` and emits that as its final event. Returns false, emitting nothing, when it is not the running one (it already ended, or was replaced).
pub(super) fn end(host: &Host, name: &'static str, serial: u64, outcome: Outcome) -> bool {
    let event = {
        let mut s = host.engine.lock();
        if s.serial != serial || s.recording != Some(name) {
            return false;
        }
        s.recording = None;
        sync_blocking(&host.engine.blocking, &s);
        done(&s, name, serial, outcome)
    };
    (host.emit)(event);
    true
}

/// Stops the running recording inside a lock the caller already holds, so a change made under that lock cannot be overwritten by the recording; returns it for `announce`.
pub(super) fn take_running(s: &mut Shared) -> Option<(&'static str, u64)> {
    s.recording.take().map(|name| (name, s.serial))
}

/// Emits the final event of a recording `take_running` stopped, once the caller has released its lock.
pub(super) fn announce(host: &Host, running: Option<(&'static str, u64)>, outcome: Outcome) {
    let Some((name, serial)) = running else { return };
    let event = done(&host.engine.lock(), name, serial, outcome);
    (host.emit)(event);
}

/// Ends whatever recording is running, if any, with `cancelled(reason)`.
pub(super) fn cancel_any(host: &Host, reason: Reason) {
    let running = {
        let s = host.engine.lock();
        s.recording.map(|name| (name, s.serial))
    };
    if let Some((name, serial)) = running {
        end(host, name, serial, Outcome::Cancelled { reason });
    }
}

/// What `commit` did with a finished combination.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Verdict {
    /// Saved; the recording stays open for the next combination.
    Saved,
    /// Refused with this message; the recording stays open for another try.
    Refused(String),
    /// The recording was no longer running; nothing happened.
    Gone,
}

/// Saves the finished combination `h` for recording `serial` of `name` when it passes the rules and tells the page with `saved`; the recording stays open, so keys pressed next replace it.
/// The combination is checked, then written to the store under the persistence lock and without the engine lock, and only a successful write changes the shortcut in memory; a failed write leaves everything as it was and the page is told, as for a refused combination.
pub(super) fn commit(host: &Host, name: &'static str, serial: u64, h: Hotkey) -> Verdict {
    let _write = host.engine.persist_lock.lock().unwrap_or_else(|e| e.into_inner());
    {
        let s = host.engine.lock();
        if s.serial != serial || s.recording != Some(name) {
            return Verdict::Gone;
        }
        if let Err(message) = matching::validate(&s.bindings, name, &h) {
            return Verdict::Refused(message);
        }
    }
    if let Err(message) = (host.persist)(name, &h) {
        return Verdict::Refused(message);
    }
    let running = {
        let mut s = host.engine.lock();
        if let Ok(b) = s.binding(name) {
            b.hotkey = h;
        }
        sync_blocking(&host.engine.blocking, &s);
        s.serial == serial && s.recording == Some(name)
    };
    (host.emit)(Event::Saved(Saved { name, serial, parts: view(&h) }));
    if running {
        Verdict::Saved
    } else {
        Verdict::Gone
    }
}

/// Runs `f` with a host wired to the app's windows and store.
pub(super) fn with_host<R>(app: &AppHandle, engine: &Engine, f: impl FnOnce(&Host) -> R) -> R {
    let emit = |event: Event| {
        if let Event::Done(Done { outcome: Outcome::Failed { message }, .. }) = &event {
            crate::status::report(app, message);
        }
        event.emit(app);
    };
    f(&Host { engine, emit: &emit, persist: &persist })
}

/// Starts recording a shortcut for the window's `request` id and returns the recording's serial; every later event carries that serial. The recording listens through the resident hook, armed on a thread of its own, so this returns at once.
/// A start whose request was already cancelled does nothing and returns an error the window ignores. A hook that cannot start ends the recording with a `failed` event carrying the reason.
#[tauri::command]
pub async fn shortcut_record(app: AppHandle, engine: State<'_, Arc<Engine>>, name: String, request: String) -> Result<u64, String> {
    let name = known(&name)?;
    let Some(serial) = with_host(&app, &engine, |host| begin(host, name, &request)) else { return Err("cancelled".into()) };
    let (arm_app, arm_engine) = (app.clone(), engine.inner().clone());
    let arm = move || {
        let _one_at_a_time = ARMING.lock().unwrap_or_else(|e| e.into_inner());
        if !current(&arm_engine, name, serial) {
            return;
        }
        let started = crate::platform::recording_listener()
            .and_then(|source| recorder::spawn(arm_app.clone(), arm_engine.clone(), name, serial, source));
        if let Err(message) = started {
            with_host(&arm_app, &arm_engine, |host| end(host, name, serial, Outcome::Failed { message }));
        }
    };
    if let Err(e) = crate::guard::spawn("shortcut-arm", 256 * 1024, arm) {
        let message = format!("Cue could not start listening for keys: {e}");
        with_host(&app, &engine, |host| end(host, name, serial, Outcome::Failed { message }));
    }
    crate::log::note(&format!("shortcut {name}: recording {serial} started"));
    Ok(serial)
}

/// Cancels the recording the window knows as `request` (a click outside, leaving the page). If it has not started yet it never will; if it ended already this does nothing.
#[tauri::command]
pub fn shortcut_record_cancel(app: AppHandle, engine: State<Arc<Engine>>, name: String, request: String) -> Result<(), String> {
    let name = known(&name)?;
    with_host(&app, &engine, |host| cancel_request(host, name, &request));
    Ok(())
}

/// The window that was recording a shortcut is gone: end the recording, or every shortcut would stay switched off.
pub fn cancel_recording(app: &AppHandle) {
    let Some(engine) = tauri::Manager::try_state::<Arc<Engine>>(app) else { return };
    with_host(app, &engine, |host| cancel_any(host, Reason::WindowClosed));
}
