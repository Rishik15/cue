//! File: shortcut/session.rs
//! Purpose: Owns who is recording: starts a recording, ends it exactly once with its one final event, saves a finished combination, and answers the settings window's commands. The serial that tells recordings apart lives here and nowhere else.
//! Contents: Host — where events and saves go; begin / end / cancel_any / commit — the recording's lifecycle (every path that ends one emits its `shortcut-done` here); shortcut_record / shortcut_record_cancel / cancel_recording — the commands and the window-closed hook.

use std::sync::Arc;

use handy_keys::Hotkey;
use tauri::{AppHandle, State};

use super::events::{Done, Event, Outcome, Reason};
use super::keys::view;
use super::{known, matching, persist, recorder, sync_blocking, Engine, Shared};

/// What a recording needs from the app: where its events go and how a saved combination is stored. The app wires the window and the store in; a test wires in memory.
pub(super) struct Host<'a> {
    pub(super) engine: &'a Engine,
    pub(super) emit: &'a dyn Fn(Event),
    pub(super) persist: &'a dyn Fn(&'static str, &Hotkey),
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

/// Starts a recording of `name` and returns its serial. A recording already running is ended with `cancelled(replaced)`.
pub(super) fn begin(host: &Host, name: &'static str) -> u64 {
    let (serial, replaced) = {
        let mut s = host.engine.lock();
        let replaced = s.recording.map(|old| done(&s, old, s.serial, Outcome::Cancelled { reason: Reason::Replaced }));
        s.serial += 1;
        s.recording = Some(name);
        sync_blocking(&host.engine.blocking, &s);
        (s.serial, replaced)
    };
    if let Some(event) = replaced {
        (host.emit)(event);
    }
    serial
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
    /// Saved, and the recording ended with `saved`.
    Saved,
    /// Refused with this message; the recording stays open for another try.
    Refused(String),
    /// The recording was no longer running; nothing happened.
    Gone,
}

/// Saves the finished combination `h` for recording `serial` of `name` when it passes the rules, and ends the recording with `saved`. Storing and emitting happen after the lock is released.
pub(super) fn commit(host: &Host, name: &'static str, serial: u64, h: Hotkey) -> Verdict {
    let verdict = {
        let mut s = host.engine.lock();
        if s.serial != serial || s.recording != Some(name) {
            return Verdict::Gone;
        }
        if let Err(message) = matching::validate(&s.bindings, name, &h) {
            return Verdict::Refused(message);
        }
        if let Ok(b) = s.binding(name) {
            b.hotkey = h;
        }
        s.recording = None;
        sync_blocking(&host.engine.blocking, &s);
        done(&s, name, serial, Outcome::Saved)
    };
    (host.persist)(name, &h);
    (host.emit)(verdict);
    Verdict::Saved
}

/// Runs `f` with a host wired to the app's windows and store.
pub(super) fn with_host<R>(app: &AppHandle, engine: &Engine, f: impl FnOnce(&Host) -> R) -> R {
    let emit = |event: Event| event.emit(app);
    let save = |name: &'static str, h: &Hotkey| persist(app, name, h);
    f(&Host { engine, emit: &emit, persist: &save })
}

/// Starts recording a shortcut and returns the recording's serial; every later event and the cancel carry it. The recording listens through a hook of its own, installed now and removed when it ends.
/// A hook that cannot start is not an error here: the recording ends at once with a `failed` event carrying the reason, so the window hears of it the same way as of any other end.
#[tauri::command]
pub fn shortcut_record(app: AppHandle, engine: State<Arc<Engine>>, name: String) -> Result<u64, String> {
    let name = known(&name)?;
    let serial = with_host(&app, &engine, |host| begin(host, name));
    let started =
        crate::platform::recording_listener().and_then(|source| recorder::spawn(app.clone(), engine.inner().clone(), name, serial, source));
    if let Err(message) = started {
        with_host(&app, &engine, |host| end(host, name, serial, Outcome::Failed { message }));
    }
    crate::log::note(&format!("shortcut {name}: recording {serial} started"));
    Ok(serial)
}

/// Cancels recording `serial` of `name` (a click outside, leaving the page). A serial that is not the running one is ignored: it belongs to a recording that already ended or was replaced.
#[tauri::command]
pub fn shortcut_record_cancel(app: AppHandle, engine: State<Arc<Engine>>, name: String, serial: u64) -> Result<(), String> {
    let name = known(&name)?;
    let ended = with_host(&app, &engine, |host| end(host, name, serial, Outcome::Cancelled { reason: Reason::Outside }));
    if !ended {
        crate::log::note(&format!("shortcut {name}: stale cancel of recording {serial} ignored"));
    }
    Ok(())
}

/// The window that was recording a shortcut is gone: end the recording, or every shortcut would stay switched off.
pub fn cancel_recording(app: &AppHandle) {
    let Some(engine) = tauri::Manager::try_state::<Arc<Engine>>(app) else { return };
    with_host(app, &engine, |host| cancel_any(host, Reason::WindowClosed));
}
