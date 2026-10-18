//! Purpose: Clipboard paste with delayed rendering: Cue owns a hidden window and supplies the text only when the
//! target app asks for it, so the original clipboard is restored right after the app has actually read it.
//! Contents: paste — snapshot, publish (plain text, and HTML when given), let monitors read first, Ctrl+V, wait for the render receipt, restore (retried, a failure is reported); wndproc — answers render requests;
//! pump_until — wait while still serving window messages.

use std::cell::{Cell, RefCell};
use std::time::{Duration, Instant};

use windows::core::w;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::DataExchange::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Ole::CF_UNICODETEXT;
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_CONTROL, VK_V};
use windows::Win32::UI::WindowsAndMessaging::*;

use super::cf_html;
use super::clipboard::{exclude_from_history, restore, set_data, snapshot, utf16_bytes, with_clipboard, PASTE_BYTES};
use super::insert::{key_chord, send, InsertError};

const READ_TIMEOUT: Duration = Duration::from_millis(1500);
const FIXED_WAIT: Duration = Duration::from_millis(250); // when a clipboard monitor already read the text, no receipt is possible
const MONITOR_SETTLE: Duration = Duration::from_millis(40); // time for clipboard monitors to read the new entry before the target does
const GRACE: Duration = Duration::from_millis(30); // let the reader finish copying and close the clipboard

thread_local! {
    static TEXT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    /// The "HTML Format" block to offer next to the text; empty when the paste is plain.
    static HTML: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    static RENDERED: Cell<bool> = const { Cell::new(false) };
}

fn html_format() -> u32 {
    // SAFETY: registering a clipboard format name returns the same id every time.
    unsafe { RegisterClipboardFormatW(w!("HTML Format")) }
}

/// Supplies one format the requester asked for.
fn render(format: u32) {
    let given = if format == html_format() {
        HTML.with(|h| set_data(format, &h.borrow()))
    } else {
        TEXT.with(|t| set_data(CF_UNICODETEXT.0 as u32, &t.borrow()))
    };
    // The receipt that the text was handed over: a failed write must not look like a paste that happened.
    if given.is_ok() {
        RENDERED.set(true);
    }
}

fn render_all() {
    render(CF_UNICODETEXT.0 as u32);
    if !HTML.with(|h| h.borrow().is_empty()) {
        render(html_format());
    }
}

/// A panic must not unwind out of the window procedure (it would abort the app), so it is contained and the message is left unhandled.
unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    crate::guard::contain(|| handle(hwnd, msg, wp, lp)).unwrap_or(LRESULT(0))
}

unsafe fn handle(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_RENDERFORMAT => {
            render(wp.0 as u32); // the requester holds the clipboard open already
            LRESULT(0)
        }
        WM_RENDERALLFORMATS => {
            // We are being destroyed while still the owner: materialise the text so it survives.
            if OpenClipboard(Some(hwnd)).is_ok() {
                if GetClipboardOwner().ok() == Some(hwnd) {
                    render_all();
                }
                let _ = CloseClipboard();
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

fn create_owner() -> Result<HWND, String> {
    unsafe {
        let module = GetModuleHandleW(None).map_err(|e| e.to_string())?;
        let class = w!("CueClipboardOwner");
        RegisterClassW(&WNDCLASSW { lpfnWndProc: Some(wndproc), hInstance: module.into(), lpszClassName: class, ..Default::default() });
        CreateWindowExW(Default::default(), class, w!(""), WS_POPUP, 0, 0, 0, 0, Some(HWND_MESSAGE), None, Some(module.into()), None)
            .map_err(|e| e.to_string())
    }
}

/// Serve window messages until `done` or the deadline, sleeping in between.
fn pump_until(deadline: Instant, done: impl Fn() -> bool) {
    let mut msg = MSG::default();
    while !done() && Instant::now() < deadline {
        let left = deadline.saturating_duration_since(Instant::now()).as_millis() as u32;
        unsafe {
            MsgWaitForMultipleObjects(None, false, left, QS_ALLINPUT);
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                DispatchMessageW(&msg);
            }
        }
    }
}

/// Put the text on the clipboard as a delayed-render entry that Windows history and cloud sync ignore.
fn publish(hwnd: HWND) -> Result<(), String> {
    with_clipboard(Some(hwnd), || unsafe {
        EmptyClipboard().map_err(|e| e.to_string())?;
        // A delayed-render entry has a null handle by design, which the bindings report as an error with code 0, so success
        // is judged by whether the format is now available.
        let _ = SetClipboardData(CF_UNICODETEXT.0 as u32, None::<HANDLE>);
        IsClipboardFormatAvailable(CF_UNICODETEXT.0 as u32).map_err(|_| "Could not publish text to the clipboard.".to_string())?;
        if !HTML.with(|h| h.borrow().is_empty()) {
            let _ = SetClipboardData(html_format(), None::<HANDLE>);
        }
        exclude_from_history()?;
        Ok(())
    })
}

/// Pastes `text`; with `html` (a fragment) the target may take the formatted version instead. The caller holds the `Lift`.
pub fn paste(text: &str, html: Option<&str>, restore_clipboard: bool) -> Result<(), InsertError> {
    // A clipboard too large to borrow is not restored afterwards: the dictated text still has to reach the app.
    let saved = if restore_clipboard { snapshot(PASTE_BYTES).ok() } else { None };
    TEXT.set(utf16_bytes(text));
    HTML.set(html.map(cf_html::build).unwrap_or_default());
    RENDERED.set(false);
    let hwnd = create_owner()?;
    // Whether the target (or a monitor) really asked for the text. When nobody did, the paste did not happen and the text must stay on the clipboard.
    let taken = Cell::new(true);
    let result = publish(hwnd).and_then(|()| {
        // Clipboard monitors (history tools) render on the change notice; let them finish first, so that a render after this point is the target's.
        pump_until(Instant::now() + MONITOR_SETTLE, || false);
        let read_early = RENDERED.get();
        RENDERED.set(false);
        send(&key_chord(VK_CONTROL, VK_V))?;
        let start = Instant::now();
        if read_early {
            pump_until(start + FIXED_WAIT, || false);
        } else {
            pump_until(start + READ_TIMEOUT, || RENDERED.get());
            taken.set(RENDERED.get());
            pump_until(Instant::now() + GRACE, || false);
        }
        Ok(())
    });
    // Restore only while the clipboard is still ours and the text was taken; a newer user copy changes the owner and wins.
    let restored = match saved.filter(|_| taken.get() && unsafe { GetClipboardOwner() }.ok() == Some(hwnd)) {
        Some(saved) => restore(saved),
        None => Ok(()),
    };
    unsafe { DestroyWindow(hwnd).ok() };
    match result {
        Err(e) => Err(e.into()),
        Ok(()) if !taken.get() => Err(InsertError::NotTaken),
        Ok(()) => restored.map_err(|_| InsertError::NotRestored),
    }
}
