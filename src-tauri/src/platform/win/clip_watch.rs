//! File: clip_watch.rs
//! Purpose: Reports text the user copies, for a note in AutoPaste mode (Windows): a hidden message-only window on its own thread that Windows tells when the clipboard changes (no polling), which exists only while a note is collecting.
//! Contents: watch — start the window and thread (or swap the handler); stop — end them; start_window / run — the window and its message loop; window_proc / copied — receive the clipboard change. A copy Cue made itself (marked to stay out of clipboard history) is ignored.

use std::sync::{Arc, Mutex};

use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::DataExchange::{
    AddClipboardFormatListener, IsClipboardFormatAvailable, RegisterClipboardFormatW, RemoveClipboardFormatListener,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;

use super::clipboard::unicode_text;
use super::watcher::{Lifecycle, Ready};

/// What is called with each copied text.
type Handler = Arc<dyn Fn(String) + Send + Sync>;

/// The current handler; None while nothing collects.
static HANDLER: Mutex<Option<Handler>> = Mutex::new(None);
/// The thread and window lifecycle.
static WATCHER: Lifecycle = Lifecycle::new();

/// Starts watching (or swaps the handler of a watch already running); `handler` gets each text the user copies. Err with a line for the user when the window cannot be made.
pub fn watch(handler: Box<dyn Fn(String) + Send + Sync>) -> Result<(), String> {
    *HANDLER.lock().unwrap_or_else(|e| e.into_inner()) = Some(handler.into());
    WATCHER.start("clip-watch", 128 * 1024, "clipboard", run).inspect_err(|_| *HANDLER.lock().unwrap_or_else(|e| e.into_inner()) = None)
}

/// Ends the watch and drops the handler.
pub fn stop() {
    *HANDLER.lock().unwrap_or_else(|e| e.into_inner()) = None;
    WATCHER.stop();
}

/// Makes the message-only window and registers it for clipboard changes. Safety: a window class and a message-only window owned by this thread, destroyed by `run`.
fn start_window() -> Result<HWND, String> {
    unsafe {
        let module = GetModuleHandleW(None).map_err(|e| e.to_string())?;
        let class = w!("CueClipboardWatcher");
        RegisterClassW(&WNDCLASSW { lpfnWndProc: Some(window_proc), hInstance: module.into(), lpszClassName: class, ..Default::default() });
        let window = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class,
            w!(""),
            WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE),
            None,
            Some(module.into()),
            None,
        )
        .map_err(|e| e.to_string())?;
        if let Err(e) = AddClipboardFormatListener(window) {
            let _ = DestroyWindow(window);
            return Err(e.to_string());
        }
        Ok(window)
    }
}

/// The watcher thread: makes the window, pumps its messages until stopped, then removes the listener and the window. Safety: only the window made here is used, on this thread.
fn run(ready: Ready) {
    let window = match start_window() {
        Ok(window) => window,
        Err(e) => return ready.failed(format!("Could not watch the clipboard: {e}")),
    };
    unsafe {
        if ready.running() {
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = DispatchMessageW(&msg);
            }
        }
        let _ = RemoveClipboardFormatListener(window);
        let _ = DestroyWindow(window);
    }
    ready.ended();
}

/// Window procedure: a clipboard update is handled under `guard::contain`, because a panic must not unwind through Windows.
unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if msg == WM_CLIPBOARDUPDATE {
        let _ = crate::guard::contain(copied);
    }
    DefWindowProcW(hwnd, msg, wp, lp)
}

/// The clipboard changed: pass its text on, unless Cue itself put it there.
/// Safety: asks whether a registered format is on the clipboard; no handle is kept.
fn copied() {
    let ours = unsafe { IsClipboardFormatAvailable(RegisterClipboardFormatW(w!("ExcludeClipboardContentFromMonitorProcessing"))) }.is_ok();
    let handler = HANDLER.lock().unwrap_or_else(|e| e.into_inner()).clone();
    if let (false, Some(handler), Some(text)) = (ours, handler, unicode_text()) {
        handler(text);
    }
}
