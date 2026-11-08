//! File: clip_watch.rs
//! Purpose: Reports text the user copies, for a note in AutoPaste mode (Windows): a hidden message-only window on its own thread that Windows tells when the clipboard changes (no polling), which exists only while a note is collecting.
//! Contents: watch — start the window and thread (or swap the handler); stop — end them; start_window / run — the window and its message loop; window_proc / copied — receive the clipboard change and read its markers, sequence number and text in one open; Provenance / provenance — who wrote it (Cue's own marker, a foreign exclusion marker, or the user); fresh — one delivery per sequence number. Copies Cue made itself or that other programs marked sensitive are not reported; copies that arrive faster than they are handled are folded into the newest, a limit of watching by events.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::DataExchange::{
    AddClipboardFormatListener, IsClipboardFormatAvailable, RegisterClipboardFormatW, RemoveClipboardFormatListener,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;

use super::clipboard::{cue_marker, sequence, text_in_open, with_clipboard_within, ANY_TEXT_BYTES};
use super::watcher::{Lifecycle, Ready};

/// What is called with each copied text.
type Handler = Arc<dyn Fn(String) + Send + Sync>;

/// The current handler; None while nothing collects.
static HANDLER: Mutex<Option<Handler>> = Mutex::new(None);
/// The thread and window lifecycle.
static WATCHER: Lifecycle = Lifecycle::new();
/// The newest clipboard sequence number handled, so one copy is delivered once.
static LAST: AtomicU32 = AtomicU32::new(0);

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
    LAST.store(sequence(), Ordering::Release);
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

/// Who wrote what is on the clipboard.
#[derive(Debug, PartialEq, Eq)]
enum Provenance {
    /// Cue itself (its own marker): never collected.
    Cue,
    /// Another program asked monitors to leave the entry alone (a password manager): not collected either.
    Sensitive,
    /// An ordinary copy by the user.
    User,
}

/// The origin from the two markers read in the same open as the text.
fn provenance(cue_marker: bool, exclusion_marker: bool) -> Provenance {
    match (cue_marker, exclusion_marker) {
        (true, _) => Provenance::Cue,
        (false, true) => Provenance::Sensitive,
        (false, false) => Provenance::User,
    }
}

/// Records `now` as the newest sequence number handled; true when it is new. Several updates queued while the handler ran all find the same newest clipboard, and only the first of them counts.
fn fresh(last: &AtomicU32, now: u32) -> bool {
    last.swap(now, Ordering::AcqRel) != now
}

/// The clipboard changed: pass its text on when it is a user copy not handled before. The markers, the text and then the sequence number are read in one open: text that carries Cue's own marker (a paste still waiting for its reader) or a foreign exclusion marker is never read, so the watcher cannot render a pending paste or take Cue's text for a note; and the sequence number comes after the read, so a delayed renderer that changes it during the read leaves the deduplication on the final value. Updates that arrive faster than they are handled are folded into the newest clipboard: the values copied in between are gone (the clipboard keeps only the last), a limit of event-based watching. Safety: asks about registered formats and reads the open clipboard; no handle is kept.
fn copied() {
    let marker = unsafe { RegisterClipboardFormatW(w!("ExcludeClipboardContentFromMonitorProcessing")) };
    let seen = with_clipboard_within(None, Duration::from_millis(200), || {
        let (cue, exclusion) = unsafe { (IsClipboardFormatAvailable(cue_marker()).is_ok(), IsClipboardFormatAvailable(marker).is_ok()) };
        let origin = provenance(cue, exclusion);
        let text = if origin == Provenance::User { text_in_open(ANY_TEXT_BYTES) } else { None };
        Ok((sequence(), origin, text))
    });
    let Ok((now, origin, text)) = seen else { return };
    let handler = HANDLER.lock().unwrap_or_else(|e| e.into_inner()).clone();
    if let (true, Provenance::User, Some(handler), Some(text)) = (fresh(&LAST, now), origin, handler, text) {
        handler(text);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicU32;

    use super::{fresh, provenance, Provenance};

    /// Cue's own marker wins; a foreign exclusion marker is kept out too; only an unmarked copy is the user's.
    #[test]
    fn only_an_unmarked_copy_is_the_users() {
        assert_eq!(provenance(true, true), Provenance::Cue);
        assert_eq!(provenance(true, false), Provenance::Cue);
        assert_eq!(provenance(false, true), Provenance::Sensitive);
        assert_eq!(provenance(false, false), Provenance::User);
    }

    /// A sequence number handled once is not handled again when more updates for it are queued.
    #[test]
    fn a_sequence_number_counts_once() {
        let last = AtomicU32::new(10);
        assert!(fresh(&last, 11));
        assert!(!fresh(&last, 11));
        assert!(fresh(&last, 12));
    }
}
