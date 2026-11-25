//! Purpose: The preview window's making and showing. It is built hidden and unfocusable at the press; the page shows it once it has something to show, and `take_focus` shows it centred on the selection's monitor and gives it the keyboard
//! when the text is copied. A watchdog closes a window whose page never reads what it should show.
//! Contents: WATCH / OWNER / FOCUS — the window guard's watchdog, the number of the window it guards and whether the window held the keyboard; warm — build the window; arm — start the watchdog; take_focus — show it (centred by place on the selection's monitor, centred_in) and give it the keyboard.

use crate::lock::lock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tauri::{AppHandle, Manager, WebviewWindowBuilder};

use crate::platform;
use crate::status;
use crate::window_guard::{self, Focus, Watch};

pub(super) const WINDOW: &str = "preview";
/// The watchdog: it ends when the page reads what to show (`preview_get`), not when the window is visible.
pub(super) static WATCH: Watch = Watch::new();
/// The number of the window now built (`window_guard::fresh`), which the watchdog names when it expires.
pub(super) static OWNER: AtomicU64 = AtomicU64::new(0);
/// The window has held the keyboard: clicking away closes the palette only after that and only when the window is no longer in front.
pub(super) static FOCUS: Focus = Focus::new();
const SHOW_DEADLINE: Duration = Duration::from_secs(10);
/// Space kept between the window and the screen edge, in logical pixels.
const EDGE_DIP: f64 = 16.0;

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
    crate::runtime::ensure();
    match WebviewWindowBuilder::from_config(app, config).and_then(|b| b.focused(false).focusable(false).build()) {
        Ok(window) => {
            let _ = window.center();
            let _ = window.set_always_on_top(true);
            OWNER.store(window_guard::fresh(), Ordering::Release);
            FOCUS.reset();
            arm(app);
        }
        Err(e) => status::report(app, &format!("Could not open the preview: {e}")),
    }
}

/// A window whose page never reads its text (a script error, a stalled webview, visible or not) would keep dictated text out of reach and make every shortcut that needs the preview refuse to start. One sleeping thread per arming closes the window it was armed for after `SHOW_DEADLINE` and rescues any dictated text. Input: the app.
pub(super) fn arm(app: &AppHandle) {
    let app = app.clone();
    let _ = WATCH.arm("preview-watch", OWNER.load(Ordering::Acquire), SHOW_DEADLINE, move |owner| super::stuck_if(&app, owner));
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
        match window_guard::handle(&window) {
            Some(handle) => {
                platform::focus_window(handle);
            }
            None => {
                let _ = window.set_focus();
            }
        }
    }
}

/// Centres the window in the work area of the monitor the selection is on (the pointer's for a grab), cutting its size first if the work area is smaller than the window; without a monitor it stays where `warm` centred it.
fn place(window: &tauri::WebviewWindow) {
    let target = lock(&super::PENDING).as_ref().map_or(0, |p| p.target.window);
    let Some((area, scale)) = platform::work_area_of(target) else { return };
    let Ok(own) = window.scale_factor() else { return };
    if let Ok(now) = window.outer_size() {
        let wanted = (f64::from(now.width) / own, f64::from(now.height) / own);
        let fitted = platform::place::fit_logical(wanted, area, f64::from(scale), EDGE_DIP);
        if fitted != wanted {
            let _ = window.set_size(tauri::LogicalSize::new(fitted.0, fitted.1));
        }
    }
    let Ok(size) = window.outer_size() else { return };
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
