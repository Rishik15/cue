//! Purpose: Reports text the user copies, for a note in AutoPaste mode (Windows): a hidden message-only window on its own thread that Windows tells when the clipboard changes (no polling), which exists only while a note is collecting.
//! Contents: watch — start the window and thread (or swap the handler); stop — end them; window_proc — receives the clipboard change. A copy Cue made itself (marked to stay out of clipboard history) is ignored.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::DataExchange::{
    AddClipboardFormatListener, IsClipboardFormatAvailable, RegisterClipboardFormatW, RemoveClipboardFormatListener,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::*;

use super::clipboard::unicode_text;

type Handler = Arc<dyn Fn(String) + Send + Sync>;

static HANDLER: Mutex<Option<Handler>> = Mutex::new(None);
/// The watcher thread's id while it runs (0 when it does not), for `stop`.
static THREAD: AtomicU32 = AtomicU32::new(0);

pub fn watch(handler: Box<dyn Fn(String) + Send + Sync>) -> Result<(), String> {
    *HANDLER.lock().unwrap_or_else(|e| e.into_inner()) = Some(handler.into());
    if THREAD.load(Ordering::Acquire) != 0 {
        return Ok(());
    }
    let (ready, started) = mpsc::channel();
    crate::guard::spawn("clip-watch", 128 * 1024, move || run(ready))
        .map_err(|e| format!("Could not start watching the clipboard: {e}"))?;
    started.recv_timeout(Duration::from_secs(2)).map_err(|_| {
        *HANDLER.lock().unwrap_or_else(|e| e.into_inner()) = None; // a thread that starts late sees no handler and ends
        "The clipboard watcher did not start.".to_string()
    })?
}

pub fn stop() {
    *HANDLER.lock().unwrap_or_else(|e| e.into_inner()) = None;
    let thread = THREAD.load(Ordering::Acquire);
    if thread != 0 {
        // SAFETY: wakes the watcher's message loop so it ends and removes its window.
        let _ = unsafe { PostThreadMessageW(thread, WM_QUIT, WPARAM(0), LPARAM(0)) };
        for _ in 0..100 {
            if THREAD.load(Ordering::Acquire) == 0 {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
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

fn run(ready: mpsc::Sender<Result<(), String>>) {
    let window = match start_window() {
        Ok(window) => window,
        Err(e) => return drop(ready.send(Err(format!("Could not watch the clipboard: {e}")))),
    };
    // SAFETY: pumps the messages of the window made above on this thread, then removes the listener and the window.
    unsafe {
        // Started too late (`watch` gave up and cleared the handler): skip the loop, only clean up.
        let wanted = HANDLER.lock().unwrap_or_else(|e| e.into_inner()).is_some();
        if wanted {
            THREAD.store(GetCurrentThreadId(), Ordering::Release);
            let _ = ready.send(Ok(()));
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = DispatchMessageW(&msg);
            }
        }
        let _ = RemoveClipboardFormatListener(window);
        let _ = DestroyWindow(window);
        if wanted {
            THREAD.store(0, Ordering::Release);
        }
    }
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
