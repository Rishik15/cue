//! Purpose: Clipboard paste with delayed rendering: Cue owns a hidden window and supplies the text only when the
//! target app asks for it, so the original clipboard is restored right after the app has actually read it.
//! Contents: paste — snapshot, publish, Ctrl+V, wait for the read, restore; wndproc — answers render requests;
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

use super::clipboard::{restore, set_data, snapshot, utf16_bytes, with_clipboard};
use super::insert::{key_chord, send};

const READ_TIMEOUT: Duration = Duration::from_millis(1500);
const FIXED_WAIT: Duration = Duration::from_millis(250); // when a clipboard monitor already read the text, no receipt is possible
const GRACE: Duration = Duration::from_millis(30); // let the reader finish copying and close the clipboard

thread_local! {
    static TEXT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    static RENDERED: Cell<bool> = const { Cell::new(false) };
}

fn render() {
    TEXT.with(|t| drop(set_data(CF_UNICODETEXT.0 as u32, &t.borrow())));
    RENDERED.set(true);
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_RENDERFORMAT => {
            render(); // the requester holds the clipboard open already
            LRESULT(0)
        }
        WM_RENDERALLFORMATS => {
            // We are being destroyed while still the owner: materialise the text so it survives.
            if OpenClipboard(Some(hwnd)).is_ok() {
                if GetClipboardOwner().ok() == Some(hwnd) {
                    render();
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
        for name in [w!("CanIncludeInClipboardHistory"), w!("CanUploadToCloudClipboard")] {
            set_data(RegisterClipboardFormatW(name), &0u32.to_le_bytes())?;
        }
        Ok(())
    })
}

pub fn paste(text: &str, restore_clipboard: bool) -> Result<(), String> {
    let saved = snapshot();
    TEXT.set(utf16_bytes(text));
    RENDERED.set(false);
    let hwnd = create_owner()?;
    let result = publish(hwnd).and_then(|()| {
        let read_early = RENDERED.get(); // a clipboard monitor asked for the text before the target did
        RENDERED.set(false);
        send(&key_chord(VK_CONTROL, VK_V))?;
        let start = Instant::now();
        if read_early {
            pump_until(start + FIXED_WAIT, || false);
        } else {
            pump_until(start + READ_TIMEOUT, || RENDERED.get());
            pump_until(Instant::now() + GRACE, || false);
        }
        Ok(())
    });
    // Restore only while the clipboard is still ours; a newer user copy changes the owner and wins.
    if restore_clipboard && unsafe { GetClipboardOwner() }.ok() == Some(hwnd) {
        restore(saved);
    }
    unsafe { DestroyWindow(hwnd).ok() };
    result
}
