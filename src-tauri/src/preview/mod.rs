//! Purpose: The preview window: after dictation it shows a finished result for review; for a selection it is the palette, a list of actions with the result beside each. It pastes or drops what the user chose.
//! A webview created when needed and destroyed when closed, so it costs nothing while unused.
//! Contents: show — a finished dictation result; open_palette_copy — a screen grab's palette (copies); open_snippets — the snippets palette; open_palette — the selection's text and its rows; warm — build the window hidden (with a watchdog that closes it if its page never shows it); context — the text the window holds and its program; dismiss — close without applying; on_focus — close the palette when the user clicks away; forget — drop the pending text when the window is gone; preview_get / preview_apply / preview_close — commands for the page.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WebviewWindowBuilder};

use crate::palette::{self, Item};
use crate::platform;
use crate::rules::{self, model::Output};
use crate::status;

use deliver::{copy, copy_with_notice, paste};

mod deliver;

const WINDOW: &str = "preview";

struct Pending {
    original: String,
    /// A finished dictation result; None for the palette, whose results come per row.
    result: Option<String>,
    /// Name of the action that made the dictation result.
    via: String,
    /// The palette's rows (a selection); None for a dictation result.
    items: Option<Vec<Item>>,
    /// The program the text came from (the foreground app at the press), for saved actions scoped to programs and for the run log.
    app: String,
    /// The window the text came from; Apply brings it back to the front before inserting.
    target: isize,
    /// Dictated text exists nowhere else, so closing the window copies it instead of dropping it.
    keep: bool,
    /// Copy instead of paste (a screen grab has no app to paste into).
    copy: bool,
    /// The snippets palette: it lists snippets with what each expands to, not actions.
    snippets: bool,
}

static PENDING: Mutex<Option<Pending>> = Mutex::new(None);
/// Which window the watchdog belongs to: a newer window must not be closed by an older window's timer.
static WATCH: AtomicU64 = AtomicU64::new(0);
/// The window has held the keyboard: clicking away closes the palette only after that, never while it is still loading unfocused.
static HAD_FOCUS: AtomicBool = AtomicBool::new(false);
const SHOW_DEADLINE: Duration = Duration::from_secs(10);

#[derive(Serialize)]
pub struct View {
    original: String,
    result: Option<String>,
    via: String,
    items: Option<Vec<Item>>,
    /// "copy" for a screen grab, "snippets" for the snippets palette, otherwise "paste".
    mode: &'static str,
}

/// Builds the window hidden, unfocused and unfocusable (the page shows it without taking focus until the text is copied). The webview is the slow part (a quarter of a second), so this starts while the selection is still being
/// copied; the page shows the window itself as soon as there is something to show.
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
            let dropped = PENDING.lock().unwrap().take();
            rescue(&app, dropped);
            dismiss(&app);
            status::report(&app, "The preview window did not open and was closed.");
        }
    });
}

/// Stores what the window shows, builds it if needed and lets it take the keyboard: the text is known, so Ctrl+C can no longer go to it by mistake.
fn publish(app: &AppHandle, pending: Pending) {
    let replaced = PENDING.lock().unwrap().replace(pending);
    rescue(app, replaced); // a dictated text still open is never overwritten silently
    warm(app);
    if app.get_webview_window(WINDOW).is_none() {
        // The window could not be built: the text has nowhere to be shown, so it goes to the clipboard.
        let stranded = PENDING.lock().unwrap().take();
        return rescue(app, stranded);
    }
    let _ = app.emit_to(WINDOW, "preview-changed", ());
    take_focus(app);
}

/// Dictated text exists nowhere else: when its entry is dropped without being applied, it goes to the clipboard.
fn rescue(app: &AppHandle, dropped: Option<Pending>) {
    if let Some(Pending { keep: true, result: Some(text), .. }) = dropped {
        copy(app, &text, "A dictated text that was still open was copied to the clipboard. Paste it with Ctrl+V.");
    }
}

/// A finished dictation result at once (the text is ready before any window exists). Closing the window copies it, so it is never lost.
pub fn show(app: &AppHandle, original: String, result: String, via: String, target: isize) {
    publish(
        app,
        Pending { original, result: Some(result), via, items: None, app: String::new(), target, keep: true, copy: false, snippets: false },
    );
}

/// The selection is copied: the window lists the actions that apply to it.
pub fn open_palette(app: &AppHandle, (original, program): (String, String), items: Vec<Item>, target: isize) {
    palette::reset();
    publish(
        app,
        Pending {
            original,
            result: None,
            via: String::new(),
            items: Some(items),
            app: program,
            target,
            keep: false,
            copy: false,
            snippets: false,
        },
    );
}

/// The snippets palette: the list of snippets, each with the text it expands to; Insert types the chosen one into the app the shortcut was pressed in.
pub fn open_snippets(app: &AppHandle, items: Vec<Item>, program: String, target: isize) {
    palette::reset();
    publish(
        app,
        Pending {
            original: String::new(),
            result: None,
            via: String::new(),
            items: Some(items),
            app: program,
            target,
            keep: false,
            copy: false,
            snippets: true,
        },
    );
}

/// The palette for a screen grab: Enter copies the chosen text.
pub fn open_palette_copy(app: &AppHandle, (original, program): (String, String), items: Vec<Item>) {
    palette::reset();
    publish(
        app,
        Pending {
            original,
            result: None,
            via: String::new(),
            items: Some(items),
            app: program,
            target: 0,
            keep: false,
            copy: true,
            snippets: false,
        },
    );
}

/// The text the window holds and the program it came from, for `preview_run`.
pub fn context() -> Option<(String, String)> {
    PENDING.lock().unwrap().as_ref().map(|p| (p.original.clone(), p.app.clone()))
}

/// The text is copied, so the window may take the keyboard now (Escape, Enter, arrows).
fn take_focus(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(WINDOW) {
        // Shown here as well as by the page: focus cannot go to a hidden window, and the page may not have run yet.
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

#[cfg(windows)]
pub(crate) fn native_handle(window: &tauri::WebviewWindow) -> Option<isize> {
    window.hwnd().ok().map(|h| h.0 as isize)
}

#[cfg(not(windows))]
pub(crate) fn native_handle(_: &tauri::WebviewWindow) -> Option<isize> {
    None
}

pub fn is_open(app: &AppHandle) -> bool {
    app.get_webview_window(WINDOW).is_some()
}

/// Focus moved to or from the window: the palette (a list of actions, like a launcher) closes when the user clicks away. A dictation result stays, since closing it would copy it unasked.
pub fn on_focus(app: &AppHandle, focused: bool) {
    if focused {
        return HAD_FOCUS.store(true, Ordering::Release);
    }
    let is_palette = PENDING.lock().unwrap().as_ref().is_some_and(|p| p.items.is_some());
    if is_palette && HAD_FOCUS.swap(false, Ordering::AcqRel) {
        dismiss(app);
    }
}

/// The window was closed by any means (its own buttons, Alt+F4): the text must not linger in memory, and dictated text is copied rather than lost.
pub fn forget(app: &AppHandle) {
    if is_open(app) {
        return; // this is the destroyed event of an earlier window; the text now pending belongs to the newer one
    }
    let dropped = PENDING.lock().unwrap().take();
    rescue(app, dropped);
}

/// Closes the window without applying anything (nothing to review after all, or the flow was cancelled).
pub fn dismiss(app: &AppHandle) {
    PENDING.lock().unwrap().take();
    if let Some(window) = app.get_webview_window(WINDOW) {
        let _ = window.destroy();
    }
}

#[tauri::command]
pub fn preview_get() -> Option<View> {
    PENDING.lock().unwrap().as_ref().map(|p| View {
        original: p.original.clone(),
        result: p.result.clone(),
        via: p.via.clone(),
        items: p.items.clone(),
        mode: if p.snippets {
            "snippets"
        } else if p.copy {
            "copy"
        } else {
            "paste"
        },
    })
}

/// The pending text once there is something to paste (a dictation result, or text from the palette's result box); otherwise Paste does nothing and the window stays.
fn take_applicable(has_text: bool) -> Option<Pending> {
    let mut pending = PENDING.lock().unwrap();
    pending.as_ref().is_some_and(|p| p.result.is_some() || (p.items.is_some() && has_text)).then(|| pending.take()).flatten()
}

/// `text` is what the result box holds (the user may have edited it); `id` is the palette row it came from. A saved action whose output is Copy copies instead of pasting.
#[tauri::command]
pub fn preview_apply(app: AppHandle, text: Option<String>, id: Option<String>) {
    let text = text.filter(|t| !t.is_empty());
    let Some(p) = take_applicable(text.is_some()) else { return };
    let Some(result) = text.or_else(|| p.result.clone()) else { return };
    dismiss(&app);
    if p.copy {
        return copy_with_notice(&app, &result);
    }
    let rule = id.as_deref().and_then(|i| i.strip_prefix("rule:")).and_then(rules::position_in);
    if let Some(at) = rule {
        rules::record_run(at, &p.app);
    }
    if let Some(snippet) = id.as_deref().and_then(|i| i.strip_prefix("snippet:")).and_then(|n| n.parse::<i64>().ok()) {
        crate::snippets::record_use(&app, snippet);
    }
    if rule.and_then(rules::output_of) == Some(Output::Copy) {
        return copy(&app, &result, "The result was copied to the clipboard.");
    }
    paste(&app, p.target, result);
}

/// Cancel: also stops the selection flow, so a result that arrives later opens nothing.
#[tauri::command]
pub fn preview_close(app: AppHandle, text: Option<String>) {
    // Dictated text is not in any document, so closing the window must not lose it (the box may hold the user's edits).
    let kept = PENDING.lock().unwrap().as_ref().filter(|p| p.keep).map(|p| p.result.clone());
    if let Some(saved) = kept.and_then(|result| text.filter(|t| !t.is_empty()).or(result)) {
        copy(&app, &saved, "The dictated text was copied to the clipboard. Paste it with Ctrl+V.");
    }
    crate::selection::cancel();
    dismiss(&app);
}
