//! File: input/bridge.rs
//! Purpose: The settings window's commands for shortcuts. The owner is the invoking window, never a payload field; a recording is opened by a synchronous call that returns its first view before any event subscription, and cancelled by its lease number in every phase. Views reach the window as `shortcut-catalog` and `shortcut-recorder` events carrying a revision or sequence, and the window asks for a fresh snapshot when it sees a gap.
//! Contents: Bridge — the settings window's owner and open recordings; shortcut_catalog / shortcut_record_open / shortcut_record_cancel / shortcut_record_regions / shortcut_resnapshot / shortcut_reset — the commands; window_closed — revokes everything when the window goes.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use tauri::{AppHandle, Manager, State, WebviewWindow};

use super::api::{Input, Owner, Recording, WindowIdentity};
use super::popup::Rect;
use super::view::{CatalogView, RecorderView};

static INCARNATION: AtomicU64 = AtomicU64::new(1);
const MAX_REGIONS: usize = 8;
const COORDINATE_LIMIT: i32 = 1_000_000;

struct Held {
    owner: Owner,
    recordings: HashMap<u64, Recording>,
}

/// The settings window's input handles.
#[derive(Default)]
pub struct Bridge(Mutex<Option<Held>>);

impl Bridge {
    /// Runs `f` on the settings window's handles, making its owner on first use. Err for any other window.
    fn with<R>(&self, input: &Input, window: &WebviewWindow, f: impl FnOnce(&mut Held) -> R) -> Result<R, String> {
        if window.label() != "settings" {
            return Err("Only the settings window may use shortcuts.".into());
        }
        let mut slot = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if slot.is_none() {
            let identity = WindowIdentity { label: window.label().into(), incarnation: INCARNATION.fetch_add(1, Ordering::AcqRel) };
            *slot = Some(Held { owner: input.owner(identity).map_err(|e| e.to_string())?, recordings: HashMap::new() });
        }
        slot.as_mut().map(f).ok_or_else(|| "The settings window is gone.".to_string())
    }
}

/// Rectangles from the window, kept only when they are few and sane.
fn validated(rects: Vec<Rect>) -> Result<Vec<Rect>, String> {
    let sane =
        |r: &Rect| r.left < r.right && r.top < r.bottom && [r.left, r.top, r.right, r.bottom].iter().all(|v| v.abs() <= COORDINATE_LIMIT);
    if rects.len() > MAX_REGIONS || !rects.iter().all(sane) {
        return Err("The shortcut control reported an invalid area.".into());
    }
    Ok(rects)
}

/// The shortcut list as it is now.
#[tauri::command]
pub async fn shortcut_catalog(input: State<'_, Input>, window: WebviewWindow) -> Result<CatalogView, String> {
    if window.label() != "settings" {
        return Err("Only the settings window may use shortcuts.".into());
    }
    Ok(input.catalog())
}

/// Opens a recording of shortcut `id` and returns its first view (phase Starting, lease number inside). Later views arrive as `shortcut-recorder` events.
#[tauri::command]
pub async fn shortcut_record_open(
    input: State<'_, Input>,
    bridge: State<'_, Bridge>,
    window: WebviewWindow,
    id: String,
) -> Result<RecorderView, String> {
    bridge.with(&input, &window, |held| {
        let recording = held.owner.record(&id).map_err(|e| e.to_string())?;
        let view = recording.snapshot();
        held.recordings.insert(view.lease, recording);
        Ok(view)
    })?
}

/// Cancels recording `lease` in whatever phase it is in and forgets it.
#[tauri::command]
pub async fn shortcut_record_cancel(
    input: State<'_, Input>,
    bridge: State<'_, Bridge>,
    window: WebviewWindow,
    lease: u64,
) -> Result<(), String> {
    bridge.with(&input, &window, |held| {
        if let Some(recording) = held.recordings.remove(&lease) {
            let _ = recording.cancel();
        }
    })
}

/// Tells recording `lease` where the control and its popover are, in physical pixels; a click outside them cancels it.
#[tauri::command]
pub async fn shortcut_record_regions(
    input: State<'_, Input>,
    bridge: State<'_, Bridge>,
    window: WebviewWindow,
    lease: u64,
    rects: Vec<Rect>,
) -> Result<(), String> {
    let rects = validated(rects)?;
    bridge.with(&input, &window, |held| {
        held.recordings.get(&lease).map(|r| r.regions(rects)).ok_or_else(|| "That recording is over.".to_string())
    })?
}

/// A fresh view of recording `lease`, for a window that saw a gap in the events.
#[tauri::command]
pub async fn shortcut_resnapshot(
    input: State<'_, Input>,
    bridge: State<'_, Bridge>,
    window: WebviewWindow,
    lease: u64,
) -> Result<RecorderView, String> {
    bridge.with(&input, &window, |held| {
        held.recordings.get(&lease).map(Recording::snapshot).ok_or_else(|| "That recording is over.".to_string())
    })?
}

/// Puts a shortcut back to its default and returns the new list once the change is saved.
#[tauri::command]
pub async fn shortcut_reset(input: State<'_, Input>, window: WebviewWindow, id: String) -> Result<CatalogView, String> {
    if window.label() != "settings" {
        return Err("Only the settings window may use shortcuts.".into());
    }
    let done = input.reset(&id);
    let answer = tauri::async_runtime::spawn_blocking(move || done.wait(Duration::from_secs(10))).await.map_err(|e| e.to_string())?;
    match answer {
        Some(Ok(_)) => Ok(input.catalog()),
        Some(Err(e)) => Err(e.to_string()),
        None => Err("Saving the shortcut took too long.".into()),
    }
}

/// The settings window closed: every recording it had ends and its owner is dropped.
pub fn window_closed(app: &AppHandle) {
    if let Some(bridge) = app.try_state::<Bridge>() {
        bridge.0.lock().unwrap_or_else(|e| e.into_inner()).take();
    }
}
