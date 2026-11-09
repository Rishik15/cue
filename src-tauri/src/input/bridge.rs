//! File: input/bridge.rs
//! Purpose: The settings window's commands for shortcuts. The owner is the invoking window, never a payload field; a recording is opened by a call that carries a request number the page chose and returns its first view, and is cancelled by that number in every phase, even before the open has answered. Views reach the window as `shortcut-catalog` and `shortcut-recorder` events carrying a revision or sequence, and the window asks for a fresh snapshot when it sees a gap.
//! Contents: Held — the open recordings by request number and the cancel watermark; Bridge — the settings window's Held; shortcut_catalog / shortcut_record_open / shortcut_record_cancel / shortcut_record_regions / shortcut_resnapshot / shortcut_reset / shortcut_recover — the commands; window_closed — revokes everything when the window goes.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use tauri::{AppHandle, Manager, State, WebviewWindow};

use super::api::{Input, Owner, Recording};
use super::popup::Rect;
use super::view::{CatalogView, RecorderView};

const MAX_REGIONS: usize = 8;
const COORDINATE_LIMIT: i32 = 1_000_000;

/// The settings window's open recordings, keyed by the request number the page chose, and the highest request it has cancelled. A cancel for a request that has not been opened yet, or whose open is still on its way, is remembered, so that open does nothing.
pub(super) struct Held {
    owner: Owner,
    recordings: HashMap<u64, Recording>,
    cancelled_upto: u64,
}

impl Held {
    /// Handles over `owner` with nothing open.
    pub(super) fn new(owner: Owner) -> Held {
        Held { owner, recordings: HashMap::new(), cancelled_upto: 0 }
    }

    /// Opens recording `request` of shortcut `id`. A request the page already cancelled returns its cancelled view without starting anything.
    pub(super) fn open(&mut self, id: &str, request: u64) -> Result<RecorderView, String> {
        if request <= self.cancelled_upto {
            return Err("That recording was cancelled.".into());
        }
        let recording = self.owner.record(id).map_err(|e| e.to_string())?;
        let view = recording.snapshot();
        self.recordings.insert(request, recording);
        Ok(view)
    }

    /// Cancels recording `request` in whatever phase it is in, and every earlier request, and forgets them.
    pub(super) fn cancel(&mut self, request: u64) {
        self.cancelled_upto = self.cancelled_upto.max(request);
        let done: Vec<u64> = self.recordings.keys().copied().filter(|r| *r <= request).collect();
        for r in done {
            if let Some(recording) = self.recordings.remove(&r) {
                let _ = recording.cancel();
            }
        }
    }

    /// The recording for `request`, if it is open.
    pub(super) fn get(&self, request: u64) -> Option<&Recording> {
        self.recordings.get(&request)
    }
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
            *slot = Some(Held::new(input.owner()));
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

/// Opens recording `request` (a number the page chose, higher than any before) of shortcut `id` and returns its first view (phase Starting, lease number inside). Later views arrive as `shortcut-recorder` events.
#[tauri::command]
pub async fn shortcut_record_open(
    input: State<'_, Input>,
    bridge: State<'_, Bridge>,
    window: WebviewWindow,
    id: String,
    request: u64,
) -> Result<RecorderView, String> {
    bridge.with(&input, &window, |held| held.open(&id, request))?
}

/// Cancels recording `request` (and every earlier one) in whatever phase it is in and forgets it. Valid before the open has answered.
#[tauri::command]
pub async fn shortcut_record_cancel(
    input: State<'_, Input>,
    bridge: State<'_, Bridge>,
    window: WebviewWindow,
    request: u64,
) -> Result<(), String> {
    bridge.with(&input, &window, |held| held.cancel(request))
}

/// Tells recording `request` where the control and its popover are, in physical pixels; a click outside them cancels it.
#[tauri::command]
pub async fn shortcut_record_regions(
    input: State<'_, Input>,
    bridge: State<'_, Bridge>,
    window: WebviewWindow,
    request: u64,
    rects: Vec<Rect>,
) -> Result<(), String> {
    let rects = validated(rects)?;
    bridge.with(&input, &window, |held| held.get(request).map(|r| r.regions(rects)).ok_or_else(|| "That recording is over.".to_string()))?
}

/// A fresh view of recording `request`, for a window that saw a gap in the events.
#[tauri::command]
pub async fn shortcut_resnapshot(
    input: State<'_, Input>,
    bridge: State<'_, Bridge>,
    window: WebviewWindow,
    request: u64,
) -> Result<RecorderView, String> {
    bridge.with(&input, &window, |held| held.get(request).map(Recording::snapshot).ok_or_else(|| "That recording is over.".to_string()))?
}

/// Puts a shortcut back to its default and returns the new list once the change is saved.
#[tauri::command]
pub async fn shortcut_reset(input: State<'_, Input>, window: WebviewWindow, id: String) -> Result<CatalogView, String> {
    if window.label() != "settings" {
        return Err("Only the settings window may use shortcuts.".into());
    }
    let done = input.reset(&id);
    let reader = input.reader();
    let answer = tauri::async_runtime::spawn_blocking(move || {
        done.wait(Duration::from_secs(10)).map(|r| r.map(|rev| reader.at(rev, Duration::from_secs(2))))
    })
    .await
    .map_err(|e| e.to_string())?;
    match answer {
        Some(Ok(view)) => Ok(view),
        Some(Err(e)) => Err(e.to_string()),
        None => Err("Saving the shortcut took too long.".into()),
    }
}

/// Reinstalls the keyboard hook and forgets key state, and returns the new list once the hook is back. The Shortcuts page offers it when the hook failed.
#[tauri::command]
pub async fn shortcut_recover(input: State<'_, Input>, window: WebviewWindow) -> Result<CatalogView, String> {
    if window.label() != "settings" {
        return Err("Only the settings window may use shortcuts.".into());
    }
    let done = input.recover();
    let answer = tauri::async_runtime::spawn_blocking(move || done.wait(Duration::from_secs(10))).await.map_err(|e| e.to_string())?;
    match answer {
        Some(Ok(())) => Ok(input.catalog()),
        Some(Err(e)) => Err(e.to_string()),
        None => Err("Restarting the keyboard shortcuts took too long.".into()),
    }
}

/// The settings window closed: every recording it had ends and its owner is dropped.
pub fn window_closed(app: &AppHandle) {
    if let Some(bridge) = app.try_state::<Bridge>() {
        bridge.0.lock().unwrap_or_else(|e| e.into_inner()).take();
    }
}
