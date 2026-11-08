//! File: paste.rs
//! Purpose: Clipboard paste in espanso's flow (write the text, Ctrl+V, restore after the app has read it) with delayed rendering on top (no wait before Ctrl+V: that gap let typed keys land between an erase and the paste): Cue owns a hidden window and supplies the text only when the
//! target app asks for it, so the original clipboard is restored right after the app has actually read it.
//! A render request counts as the target's only when the program that has the clipboard open is the target's (same top-level window or process): a history tool or a VM's clipboard agent reading first, at any speed, is not a paste.
//! Windows asks the owner to render a format once; after a clipboard agent has taken it the text is stored and the target reads it without asking, so an agent's read means the target will not ask: the paste is then counted as done, and the old clipboard is not put back (nothing says when the target has read).
//! Contents: paste — borrow the clipboard, publish (plain text, and HTML when given), erase and Ctrl+V in one batch, wait for the target's render receipt, wait until nothing has asked for 100 ms, restore only while the clipboard is still Cue's (retried, a failure is reported); requested_by_target — who is reading; wndproc / handle — answer render requests;
//! render / render_all / publish — what the owner window offers; create_owner — the hidden owner window; pump_until / wait_quiet — wait while still serving window messages.

use std::cell::{Cell, RefCell};
use std::time::{Duration, Instant};

use windows::core::w;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::DataExchange::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Ole::CF_UNICODETEXT;
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_BACK, VK_CONTROL, VK_V};
use windows::Win32::UI::WindowsAndMessaging::*;

use super::borrow::{Borrow, Ours};
use super::cf_html;
use super::clipboard::{exclude_from_history, set_data, utf16_bytes, with_clipboard};
use super::insert::{key_chord, send, taps};
use crate::platform::InsertError;

/// How long the target gets to ask for the pasted text.
const READ_TIMEOUT: Duration = Duration::from_millis(1500);
/// After the target has asked, how long nothing may ask again before the old clipboard goes back: a reader that takes the text and then the HTML (or asks twice) must not get the old clipboard in between.
const QUIET: Duration = Duration::from_millis(100);
/// The longest the old clipboard waits for a reader that keeps asking.
const QUIET_CAP: Duration = Duration::from_millis(1000);
/// After an agent has rendered the text, how long the target still gets to ask before its silence is read as "it took the stored copy".
const CACHED_SETTLE: Duration = Duration::from_millis(300);

thread_local! {
    /// The UTF-16 text to offer to the reader.
    static TEXT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    /// The "HTML Format" block to offer next to the text; empty when the paste is plain.
    static HTML: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    /// Set when the target itself was handed the text.
    static RENDERED: Cell<bool> = const { Cell::new(false) };
    /// Set when someone else (a clipboard agent or history tool) was handed the text.
    static CACHED: Cell<bool> = const { Cell::new(false) };
    /// The top-level window and process the paste is for; (0, 0) when Windows named none, so any reader counts.
    static TARGET: Cell<(isize, u32)> = const { Cell::new((0, 0)) };
    /// When the text was last handed to any reader.
    static LAST_READ: Cell<Option<Instant>> = const { Cell::new(None) };
}

/// A window's top-level window and its process; a window that is gone gives zeros. Safety: `GetWindowThreadProcessId` writes the process id into the local `pid`, which outlives the call; `GetAncestor` only reads window data and gives a null handle for a window that is gone.
fn identity(window: HWND) -> (isize, u32) {
    let mut pid = 0;
    unsafe {
        GetWindowThreadProcessId(window, Some(&mut pid));
        (GetAncestor(window, GA_ROOT).0 as isize, pid)
    }
}

/// Whether the program reading the clipboard right now (the window that has it open at this moment) is the paste's target. A browser or an Electron app reads through a helper window of the same process, a packaged app through a child window of the same top-level one; a reader that cannot be named counts (nothing to tell it apart by). Safety: `GetOpenClipboardWindow` takes no arguments; the handle it returns is only compared and passed to `identity`.
fn requested_by_target() -> bool {
    let (root, pid) = TARGET.get();
    let Ok(reader) = (unsafe { GetOpenClipboardWindow() }) else { return true };
    if root == 0 {
        return true;
    }
    let (reader_root, reader_pid) = identity(reader);
    reader_root == root || (pid != 0 && reader_pid == pid)
}

/// The registered clipboard format id of "HTML Format". Safety: registering a clipboard format name returns the same id every time; the name is a static wide string.
fn html_format() -> u32 {
    unsafe { RegisterClipboardFormatW(w!("HTML Format")) }
}

/// Supplies one format the requester asked for. The receipt that the text was handed to the target is recorded only for a successful write by the target itself: a failed write, or a reader that is not the target, must not look like a paste that happened.
fn render(format: u32) {
    let given = if format == html_format() {
        HTML.with(|h| set_data(format, &h.borrow()))
    } else {
        TEXT.with(|t| set_data(CF_UNICODETEXT.0 as u32, &t.borrow()))
    };
    if given.is_ok() {
        LAST_READ.set(Some(Instant::now()));
        if requested_by_target() {
            RENDERED.set(true);
        } else {
            CACHED.set(true);
        }
    }
}

/// Supplies the text, and the HTML when the paste has any.
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

/// Answers the two clipboard-owner messages. On `WM_RENDERFORMAT` the requester holds the clipboard open already; on `WM_RENDERALLFORMATS` the owner is being destroyed while still the owner, so the text is materialised to survive.
unsafe fn handle(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_RENDERFORMAT => {
            render(wp.0 as u32);
            LRESULT(0)
        }
        WM_RENDERALLFORMATS => {
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

/// Creates the hidden message-only window that owns the clipboard while the text is offered. Safety: the window class and window are created on this thread with a window procedure that never unwinds (`wndproc`); the caller destroys the window.
fn create_owner() -> Result<HWND, String> {
    unsafe {
        let module = GetModuleHandleW(None).map_err(|e| e.to_string())?;
        let class = w!("CueClipboardOwner");
        RegisterClassW(&WNDCLASSW { lpfnWndProc: Some(wndproc), hInstance: module.into(), lpszClassName: class, ..Default::default() });
        CreateWindowExW(Default::default(), class, w!(""), WS_POPUP, 0, 0, 0, 0, Some(HWND_MESSAGE), None, Some(module.into()), None)
            .map_err(|e| e.to_string())
    }
}

/// Serve window messages until `done` or the deadline, sleeping in between. Safety: only messages of windows created on this thread are dispatched (the clipboard owner), and `msg` is a local that outlives each call.
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

/// After someone has read the text: wait until nothing has asked for `QUIET`, at most `QUIET_CAP`. Nothing is waited for when nobody asked.
fn wait_quiet() {
    let cap = Instant::now() + QUIET_CAP;
    while Instant::now() < cap && LAST_READ.get().is_some_and(|t| t.elapsed() < QUIET) {
        pump_until(Instant::now() + Duration::from_millis(10), || false);
    }
}

/// Puts the text on the clipboard as a delayed-render entry that Windows history and cloud sync ignore. A delayed-render entry has a null handle by design, which the bindings report as an error with code 0, so success is judged by whether the format is now available. Safety: runs while the clipboard is open for `hwnd`; the null handles are the delayed-render requests, and nothing is dereferenced.
fn publish(hwnd: HWND) -> Result<(), String> {
    with_clipboard(Some(hwnd), || unsafe {
        EmptyClipboard().map_err(|e| e.to_string())?;
        let _ = SetClipboardData(CF_UNICODETEXT.0 as u32, None::<HANDLE>);
        IsClipboardFormatAvailable(CF_UNICODETEXT.0 as u32).map_err(|_| "Could not publish text to the clipboard.".to_string())?;
        if !HTML.with(|h| h.borrow().is_empty()) {
            let _ = SetClipboardData(html_format(), None::<HANDLE>);
        }
        exclude_from_history()?;
        Ok(())
    })
}

/// Pastes `text` over the `erase` characters before the caret; with `html` (a fragment) the target may take the formatted version instead. The caller holds the `Lift`.
/// The old clipboard is borrowed first; one too large to borrow is not restored afterwards, because the dictated text still has to reach the app. The erase of a typed trigger and the Ctrl+V are one batch of input, so keys the user types meanwhile cannot land between them, with no wait before it: a clipboard manager that reads first is told apart from the target by the render receipt.
/// Safety: `GetForegroundWindow` only names a window; `DestroyWindow` ends the owner window created here, on this thread, once, after the clipboard was released. The old clipboard goes back only if the target asked for the text and the clipboard is still owned by Cue's window when it is opened for the restore; a newer user copy changes the owner and wins. When nobody asked, the paste did not happen and the text stays on the clipboard.
pub fn paste(text: &str, html: Option<&str>, restore_clipboard: bool, erase: usize) -> Result<(), InsertError> {
    let borrow = restore_clipboard.then(|| Borrow::begin().ok()).flatten();
    TEXT.set(utf16_bytes(text));
    HTML.set(html.map(cf_html::build).unwrap_or_default());
    RENDERED.set(false);
    CACHED.set(false);
    LAST_READ.set(None);
    let hwnd = create_owner()?;
    let taken = Cell::new(true);
    let result = publish(hwnd).and_then(|()| {
        TARGET.set(identity(unsafe { GetForegroundWindow() }));
        let mut keys = taps(VK_BACK, erase);
        keys.extend(key_chord(VK_CONTROL, VK_V));
        send(&keys)?;
        pump_until(Instant::now() + READ_TIMEOUT, || RENDERED.get() || CACHED.get());
        if !RENDERED.get() && CACHED.get() {
            pump_until(Instant::now() + CACHED_SETTLE, || RENDERED.get());
        }
        taken.set(RENDERED.get() || CACHED.get());
        wait_quiet();
        Ok(())
    });
    let restored = match borrow.filter(|_| RENDERED.get()) {
        Some(borrow) => borrow.finish(Ours::Owner(hwnd)).map(|_| ()),
        None => Ok(()),
    };
    unsafe { DestroyWindow(hwnd).ok() };
    match result {
        Err(e) => Err(e.into()),
        Ok(()) if !taken.get() => Err(InsertError::NotTaken),
        Ok(()) => restored.map_err(|_| InsertError::NotRestored),
    }
}
