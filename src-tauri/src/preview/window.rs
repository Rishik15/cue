//! Purpose: The preview window's making and showing. It is built hidden and unfocusable at the press; the page shows it once it has something to show, and `take_focus` shows it centred on the selection's monitor and gives it the keyboard
//! when the text is copied. A watchdog closes a window whose page never shows up.
//! Contents: warm — build the window; take_focus — show it (centred by place on the selection's monitor, centred_in) and give it the keyboard; native_handle — its Win32 handle.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use tauri::{AppHandle, Manager, WebviewWindowBuilder};

use crate::platform;
use crate::status;

pub(super) const WINDOW: &str = "preview";
/// Which window the watchdog belongs to: a newer window must not be closed by an older window's timer.
static WATCH: AtomicU64 = AtomicU64::new(0);
/// The window has held the keyboard: clicking away closes the palette only after that, never while it is still loading unfocused.
pub(super) static HAD_FOCUS: AtomicBool = AtomicBool::new(false);
const SHOW_DEADLINE: Duration = Duration::from_secs(10);

pub(super) fn exists(app: &AppHandle) -> bool {
    app.get_webview_window(WINDOW).is_some()
}

/// Builds the window hidden, unfocused and unfocusable (it must not take the focus before the text is copied, or Ctrl+C would go to it). The webview is the slow part (a quarter of a second), so this is called as early as the flow can.
pub fn warm(app: &AppHandle) {
    if app.get_webview_window(WINDOW).is_some() {
        return;
    }
    let Some(config) = app.config().app.windows.iter().find(|w| w.label == WINDOW) else { return };
    // Created from the config thread-safely: the caller is a worker thread, never the event loop.
    match WebviewWindowBuilder::from_config(app, config).and_then(|b| b.focused(false).focusable(false).build()) {
        Ok(window) => {
            let _ = window.center();
            let _ = window.set_always_on_top(true);
            HAD_FOCUS.store(false, Ordering::Release);
            watch(app);
        }
        Err(e) => status::report(app, &format!("Could not open the preview: {e}")),
    }
}

/// A window whose page never shows itself (a script error, a stalled webview) would sit hidden and make every shortcut that needs the preview refuse to start. One sleeping thread per
/// window checks after a while and, if it is still hidden, closes it and rescues any dictated text.
fn watch(app: &AppHandle) {
    let me = WATCH.fetch_add(1, Ordering::AcqRel) + 1;
    let app = app.clone();
    let _ = crate::guard::spawn("preview-watch", 64 * 1024, move || {
        std::thread::sleep(SHOW_DEADLINE);
        let hidden = app.get_webview_window(WINDOW).is_some_and(|w| !w.is_visible().unwrap_or(true));
        if hidden && WATCH.load(Ordering::Acquire) == me {
            super::stuck(&app);
        }
    });
}

/// The text is copied, so the window is shown and may take the keyboard (Escape, Enter, arrows).
pub(super) fn take_focus(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(WINDOW) {
        place(&window);
        // Focus cannot go to a hidden window, so it is shown first.
        let _ = window.show();
        let _ = window.set_focusable(true);
        let _ = window.set_always_on_top(true);
        // The app we returned the focus to must not end up above the preview. Tauri's own `set_focus` is a plain SetForegroundWindow, which Windows refuses from a background process; the platform call works around that.
        match native_handle(&window) {
            Some(handle) => {
                platform::focus_window(handle);
            }
            None => {
                let _ = window.set_focus();
            }
        }
    }
}

/// Centres the window in the work area of the monitor the selection is on (the pointer's for a grab); without one it stays where `warm` centred it.
fn place(window: &tauri::WebviewWindow) {
    let target = super::PENDING.lock().unwrap().as_ref().map_or(0, |p| p.target);
    let Some((area, scale)) = platform::work_area_of(target) else { return };
    let (Ok(size), Ok(own)) = (window.outer_size(), window.scale_factor()) else { return };
    let grown = |px: u32| (f64::from(px) / own * f64::from(scale)).round() as i32;
    let (x, y) = centred_in(area, (grown(size.width), grown(size.height)));
    let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
}

/// Top-left that centres a window of `size` in `area`; a window larger than the area sits at the area's top-left.
fn centred_in(area: platform::Rect, size: (i32, i32)) -> (i32, i32) {
    let x = area.left + ((area.right - area.left - size.0) / 2).max(0);
    let y = area.top + ((area.bottom - area.top - size.1) / 2).max(0);
    (x, y)
}

#[cfg(windows)]
pub(crate) fn native_handle(window: &tauri::WebviewWindow) -> Option<isize> {
    window.hwnd().ok().map(|h| h.0 as isize)
}

#[cfg(not(windows))]
pub(crate) fn native_handle(_: &tauri::WebviewWindow) -> Option<isize> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(left: i32, top: i32, right: i32, bottom: i32) -> platform::Rect {
        platform::Rect { left, top, right, bottom }
    }

    #[test]
    fn centres_on_any_origin() {
        assert_eq!(centred_in(area(1920, 0, 3840, 1040), (800, 400)), (2480, 320));
        assert_eq!(centred_in(area(-1920, -100, 0, 980), (800, 400)), (-1360, 240));
    }

    #[test]
    fn clamps_to_the_top_left_when_larger() {
        assert_eq!(centred_in(area(-1280, 50, -480, 450), (900, 500)), (-1280, 50));
    }
}
