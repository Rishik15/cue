//! Purpose: Reports text the user copies, for a note in AutoPaste mode (Windows): a hidden message-only window on its own thread that Windows tells when the clipboard changes (no polling), which exists only while a note is collecting.
//! Contents: watch — start the window and thread (or swap the handler); stop — end them; window_proc — receives the clipboard change. A copy Cue made itself (marked to stay out of clipboard history) is ignored.

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

type Handler = Arc<dyn Fn(String) + Send + Sync>;

static HANDLER: Mutex<Option<Handler>> = Mutex::new(None);
static WATCHER: Lifecycle = Lifecycle::new();

pub fn watch(handler: Box<dyn Fn(String) + Send + Sync>) -> Result<(), String> {
    *HANDLER.lock().unwrap_or_else(|e| e.into_inner()) = Some(handler.into());
    WATCHER.start("clip-watch", 128 * 1024, "clipboard", run).inspect_err(|_| *HANDLER.lock().unwrap_or_else(|e| e.into_inner()) = None)
}

pub fn stop() {
    *HANDLER.lock().unwrap_or_else(|e| e.into_inner()) = None;
    WATCHER.stop();
}

fn start_window() -> Result<HWND, String> {
    // SAFETY: a window class and a message-only window owned by this thread, destroyed by `run`.
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

fn run(ready: Ready) {
    let window = match start_window() {
        Ok(window) => window,
        Err(e) => return ready.failed(format!("Could not watch the clipboard: {e}")),
    };
    // SAFETY: pumps the messages of the window made above on this thread, then removes the listener and the window.
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

unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if msg == WM_CLIPBOARDUPDATE {
        // A panic must not unwind through Windows.
        let _ = crate::guard::contain(copied);
    }
    DefWindowProcW(hwnd, msg, wp, lp)
}

/// The clipboard changed: pass its text on, unless Cue itself put it there.
fn copied() {
    // SAFETY: asks whether a registered format is on the clipboard; no handle is kept.
    let ours = unsafe { IsClipboardFormatAvailable(RegisterClipboardFormatW(w!("ExcludeClipboardContentFromMonitorProcessing"))) }.is_ok();
    let handler = HANDLER.lock().unwrap_or_else(|e| e.into_inner()).clone();
    if let (false, Some(handler), Some(text)) = (ours, handler, unicode_text()) {
        handler(text);
    }
}
