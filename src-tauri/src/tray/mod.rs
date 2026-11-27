//! File: tray/mod.rs
//! Purpose: Tray icon and settings-window lifecycle. Closing the settings window destroys it (frees the webview);
//! Cue keeps running in the tray so the global shortcut stays active.
//! Contents: init — builds the tray (right click opens the popup menu from `menu`, left click opens settings);
//! SettingsWindow — the window's open lock, close tracker and pending page, one per app (managed by `lib::run`, never a static); Closing — the close-in-progress flag and its condvar; open_settings / open_settings_at — under one lock (open_with), focus the window or build it (build) from the config, after a closing one is gone (wait_for_close; with an update waiting, every open lands on About); request_page / take_start_page — the page the window was asked to show, kept until it takes it; note_window_event — tells it when the settings window starts and finishes closing.

mod menu;
#[cfg(test)]
mod tests;

use crate::lock::lock;
use std::sync::{mpsc, Condvar, Mutex};
use std::time::{Duration, Instant};

use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, WebviewWindowBuilder, WindowEvent};

use crate::platform::Point;

const WINDOW: &str = "settings";
pub const TRAY_ID: &str = "main";

/// Whether the settings window is closing (asked to close, not yet destroyed), with a condvar the Destroyed event signals.
#[derive(Default)]
pub(crate) struct Closing {
    state: Mutex<bool>,
    done: Condvar,
}

impl Closing {
    /// Marks a close as started.
    fn begin(&self) {
        *lock(&self.state) = true;
    }

    /// Marks the close as finished and wakes every waiter.
    fn end(&self) {
        *lock(&self.state) = false;
        self.done.notify_all();
    }

    /// Blocks while a close is under way, for at most `limit`. Returns whether it had to wait (a close was under way on entry).
    fn wait(&self, limit: Duration) -> bool {
        let guard = lock(&self.state);
        if !*guard {
            return false;
        }
        let _ = self.done.wait_timeout_while(guard, limit, |closing| *closing);
        true
    }
}

/// The settings window's state, held by the app: whether it is closing, the lock that serializes opens, and the page it was asked to show.
#[derive(Default)]
pub struct SettingsWindow {
    /// The settings window was asked to close and is not destroyed yet.
    closing: Closing,
    /// Serializes settings opens: a second request waits for the first, then finds the window and focuses it, so there is one build.
    open: Mutex<()>,
    /// The page the settings window has been asked to show and has not taken yet; the latest request wins.
    start_page: Mutex<Option<&'static str>>,
}

impl SettingsWindow {
    /// Records the page the window must show; it stays until the window takes it, so a window whose page has not started listening still gets it. Input: the page, None leaves the request as it is.
    fn request_page(&self, page: Option<&'static str>) {
        if page.is_some() {
            *lock(&self.start_page) = page;
        }
    }

    /// The page the window was asked to show, once. Returns it, or None.
    fn take_page(&self) -> Option<&'static str> {
        lock(&self.start_page).take()
    }

    /// Takes the open lock, waits out a close, then focuses the existing window or builds one. Inputs: how to wait for a close, how to find the window, how to focus it, how to build it.
    fn open_with<W>(&self, wait: impl FnOnce(), exists: impl Fn() -> Option<W>, focus: impl FnOnce(W), build: impl FnOnce()) {
        let _open = lock(&self.open);
        wait();
        match exists() {
            Some(window) => focus(window),
            None => build(),
        }
    }
}

/// Called for every window event. Creating a window while the previous settings window is still being destroyed re-enters the event loop inside the
/// runtime's window table and panics ("already borrowed", seen as a crash when reopening at once), so `open_settings` waits for the close to end.
/// Inputs: the app handle, the window's label and the event.
pub fn note_window_event(app: &AppHandle, label: &str, event: &WindowEvent) {
    if label != WINDOW {
        return;
    }
    let Some(state) = app.try_state::<SettingsWindow>() else { return };
    match event {
        WindowEvent::CloseRequested { .. } => state.closing.begin(),
        WindowEvent::Destroyed => state.closing.end(),
        _ => {}
    }
}

/// Waits for a close in progress (signalled by the Destroyed event), then for one turn of the event loop, so the destroy has fully returned to it. Never call it from the main thread: the close can only finish there.
/// Inputs: the app handle and the window's close tracker.
fn wait_for_close(app: &AppHandle, closing: &Closing) {
    if !closing.wait(Duration::from_secs(10)) {
        return;
    }
    let (tx, rx) = mpsc::channel();
    if app
        .run_on_main_thread(move || {
            let _ = tx.send(());
        })
        .is_ok()
    {
        let _ = rx.recv_timeout(Duration::from_secs(5));
    }
}

/// The page the window was asked to show, once (the window calls it when its listener is ready and on every `cue-goto`). Input: the window state. Returns the page, or None.
#[tauri::command]
pub fn take_start_page(state: tauri::State<'_, SettingsWindow>) -> Option<&'static str> {
    state.take_page()
}

/// Opening the settings while an update waits starts on About, until it is installed.
pub fn open_settings(app: &AppHandle) {
    open_settings_at(app, crate::update::ready_version(app).is_some().then_some("about"));
}

/// Brings the window forward: records `page` if given and tells the window, restores it if minimized, shows and focuses it.
fn focus(app: &AppHandle, state: &SettingsWindow, window: &tauri::WebviewWindow, page: Option<&'static str>) {
    if page.is_some() {
        state.request_page(page);
        let _ = app.emit_to(WINDOW, "cue-goto", ());
    }
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
}

/// Builds the settings window from the config, reporting a failure to the user.
fn build(app: &AppHandle) {
    let Some(config) = app.config().app.windows.iter().find(|w| w.label == WINDOW) else { return };
    crate::runtime::ensure();
    let began = Instant::now();
    match WebviewWindowBuilder::from_config(app, config).and_then(|b| b.build()) {
        Ok(_) => crate::log::note(&format!("settings: window built in {} ms", began.elapsed().as_millis())),
        Err(e) => {
            crate::log::note(&format!("tray: the settings window could not be built: {e}"));
            crate::status::report(app, &format!("The settings window could not open: {e}"));
        }
    }
}

/// Focuses the window or creates it, first waiting out a close in progress (inside the open lock) so a dying window is never the one focused; `page` is where it should be (kept until the window takes it). Never call it from the main thread.
pub fn open_settings_at(app: &AppHandle, page: Option<&'static str>) {
    let state = app.state::<SettingsWindow>();
    state.open_with(
        || wait_for_close(app, &state.closing),
        || app.get_webview_window(WINDOW),
        |window| focus(app, &state, &window, page),
        || {
            state.request_page(page);
            build(app);
        },
    );
}

/// The menu runs its own message loop until it closes, so it gets a short-lived thread instead of blocking the tray's
/// window procedure (and the settings window it may open needs a free event loop).
pub fn init(app: &AppHandle) -> tauri::Result<()> {
    crate::log::stage("tray: decoding the icon");
    let icon = tauri::image::Image::from_bytes(include_bytes!("../../icons/tray.png"))?;
    crate::log::stage("tray: icon decoded, building");
    let tray = TrayIconBuilder::with_id(TRAY_ID).icon(icon).tooltip("Cue").on_tray_icon_event(|tray, event| {
        let TrayIconEvent::Click { button, button_state: MouseButtonState::Up, position, .. } = event else { return };
        let app = tray.app_handle().clone();
        let handle = app.clone();
        crate::guard::spawn_or_report(&handle, "tray-click", 512 * 1024, "The tray menu", move || match button {
            MouseButton::Left => open_settings(&app),
            MouseButton::Right => menu::open(&app, Point { x: position.x as i32, y: position.y as i32 }),
            _ => {}
        });
    });
    tray.build(app)?;
    crate::log::stage("tray: built");
    Ok(())
}
