//! File: owner.rs
//! Purpose: The message-only window that owns the clipboard while Cue writes it. Windows requires a real owner for a write that empties the clipboard, and the owner is also what serves delayed-render requests and tells a reader that is the target from one that is an agent.
//! Contents: Owner — the window (new, hwnd, offer, rendered, cached, html_pending, lost, read_within, still_owns, pump_until, materialize, and listen / updates / drain for hearing clipboard changes); Listening — the change registration; Offer — what a delayed-render paste hands out (thread-local, because the window procedure runs on the thread that made the window).
//! A reader that cannot be named (it opened the clipboard with no window) counts as an agent, never as the target: its read must not look like the paste (ADR 0014).
//! Who is reading (the target, an agent, or Cue's own watcher) is decided by `reader::classify`.

use std::cell::RefCell;
use std::time::{Duration, Instant};

use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::DataExchange::{
    AddClipboardFormatListener, GetClipboardOwner, GetOpenClipboardWindow, RemoveClipboardFormatListener,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Ole::CF_UNICODETEXT;
use windows::Win32::UI::WindowsAndMessaging::*;

use super::super::clipboard::{html_format, open_within, set_data};
use super::reader::{classify, Reader};

/// What a delayed-render paste offers and what happened to it.
#[derive(Default)]
struct Offer {
    text: Vec<u8>,
    html: Vec<u8>,
    pending_text: bool,
    pending_html: bool,
    rendered: bool,
    cached: bool,
    lost: bool,
    updates: u32,
    last_read: Option<Instant>,
    target: (isize, u32),
}

thread_local! {
    static OFFER: RefCell<Offer> = RefCell::new(Offer::default());
}

/// The clipboard owner window; destroyed on drop.
pub struct Owner {
    hwnd: HWND,
}

/// A window's top-level window and its process; a window that is gone gives zeros. Safety: `GetWindowThreadProcessId` writes the process id into the local `pid`; `GetAncestor` only reads window data.
pub fn identity(window: HWND) -> (isize, u32) {
    let mut pid = 0;
    unsafe {
        GetWindowThreadProcessId(window, Some(&mut pid));
        (GetAncestor(window, GA_ROOT).0 as isize, pid)
    }
}

/// Who has the clipboard open right now. Safety: `GetOpenClipboardWindow` takes no arguments; the handle is only looked at.
fn current_reader(target: (isize, u32)) -> Reader {
    let reader = unsafe { GetOpenClipboardWindow() }.ok().map(identity);
    classify(reader, target, std::process::id())
}

/// Supplies one format a reader asked for, and records who asked: a failed write, or a reader that is not the target, must not look like a paste that happened.
fn render(format: u32) {
    let (bytes, is_html) = OFFER.with(|o| {
        let o = o.borrow();
        if format == html_format() {
            (o.html.clone(), true)
        } else {
            (o.text.clone(), false)
        }
    });
    if set_data(if is_html { format } else { CF_UNICODETEXT.0 as u32 }, &bytes).is_err() {
        return;
    }
    let target = OFFER.with(|o| o.borrow().target);
    let reader = current_reader(target);
    OFFER.with(|o| {
        let mut o = o.borrow_mut();
        if is_html {
            o.pending_html = false;
        } else {
            o.pending_text = false;
        }
        o.last_read = Some(Instant::now());
        match reader {
            Reader::Target => o.rendered = true,
            Reader::Agent => o.cached = true,
            Reader::Cue => {}
        }
    });
}

/// Supplies whatever is still pending, in the open clipboard.
fn render_pending() {
    let (text, html) = OFFER.with(|o| (o.borrow().pending_text, o.borrow().pending_html));
    if text {
        render(CF_UNICODETEXT.0 as u32);
    }
    if html {
        render(html_format());
    }
}

/// A panic must not unwind out of the window procedure (it would abort the app), so it is contained and the message is left unhandled.
unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    crate::guard::contain(|| handle(hwnd, msg, wp, lp)).unwrap_or(LRESULT(0))
}

/// Answers the clipboard-owner messages. `WM_RENDERFORMAT` arrives with the requester holding the clipboard; `WM_DESTROYCLIPBOARD` means someone emptied it, so nothing pending can be served any more.
unsafe fn handle(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_RENDERFORMAT => {
            render(wp.0 as u32);
            LRESULT(0)
        }
        WM_RENDERALLFORMATS => {
            if open_within(Some(hwnd), Duration::from_millis(500)).is_ok() {
                if GetClipboardOwner().ok() == Some(hwnd) {
                    render_pending();
                }
                let _ = windows::Win32::System::DataExchange::CloseClipboard();
            }
            LRESULT(0)
        }
        WM_CLIPBOARDUPDATE => {
            OFFER.with(|o| o.borrow_mut().updates += 1);
            LRESULT(0)
        }
        WM_DESTROYCLIPBOARD => {
            OFFER.with(|o| {
                let mut o = o.borrow_mut();
                o.lost = true;
                o.pending_text = false;
                o.pending_html = false;
            });
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

impl Owner {
    /// Creates the hidden window on this thread and clears any earlier offer. Safety: the class and window are made on this thread with a window procedure that never unwinds; `Drop` destroys the window.
    pub fn new() -> Result<Owner, String> {
        OFFER.with(|o| *o.borrow_mut() = Offer::default());
        unsafe {
            let module = GetModuleHandleW(None).map_err(|e| e.to_string())?;
            let class = w!("CueClipboardOwner");
            RegisterClassW(&WNDCLASSW { lpfnWndProc: Some(wndproc), hInstance: module.into(), lpszClassName: class, ..Default::default() });
            CreateWindowExW(Default::default(), class, w!(""), WS_POPUP, 0, 0, 0, 0, Some(HWND_MESSAGE), None, Some(module.into()), None)
                .map(|hwnd| Owner { hwnd })
                .map_err(|e| e.to_string())
        }
    }

    /// The window handle, for `OpenClipboard` and ownership checks.
    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    /// Sets what a delayed-render paste hands out: the UTF-16 text, the HTML block (empty for none) and the (top-level window, process) of the target.
    pub fn offer(&self, text: Vec<u8>, html: Vec<u8>, target: (isize, u32)) {
        OFFER.with(|o| {
            *o.borrow_mut() = Offer { pending_text: true, pending_html: !html.is_empty(), text, html, target, ..Offer::default() };
        });
    }

    /// Whether the offer carries an HTML block next to the text.
    pub fn offers_html(&self) -> bool {
        OFFER.with(|o| !o.borrow().html.is_empty())
    }

    /// Whether the target itself was handed the text.
    pub fn rendered(&self) -> bool {
        OFFER.with(|o| o.borrow().rendered)
    }

    /// Whether some other reader (a history tool or clipboard agent, or one that could not be named) was handed the text.
    pub fn cached(&self) -> bool {
        OFFER.with(|o| o.borrow().cached)
    }

    /// Whether anyone emptied the clipboard after the offer, so the offer can no longer be served.
    pub fn lost(&self) -> bool {
        OFFER.with(|o| o.borrow().lost)
    }

    /// Whether the clipboard still holds this offer, asked right now: waiting messages are served first (a copy by someone else arrives as one), then the owner window is compared. Safety: `GetClipboardOwner` takes no arguments.
    pub fn still_owns(&self) -> bool {
        self.drain();
        !self.lost() && unsafe { GetClipboardOwner() }.ok() == Some(self.hwnd)
    }

    /// Whether the HTML block was offered and nobody has asked for it yet.
    pub fn html_pending(&self) -> bool {
        OFFER.with(|o| o.borrow().pending_html)
    }

    /// Whether any reader was handed the text less than `quiet` ago.
    pub fn read_within(&self, quiet: Duration) -> bool {
        OFFER.with(|o| o.borrow().last_read.is_some_and(|t| t.elapsed() < quiet))
    }

    /// Serves window messages until `done` or the deadline, sleeping in between. Safety: only messages of windows created on this thread are dispatched, and `msg` outlives each call.
    pub fn pump_until(&self, deadline: Instant, done: impl Fn() -> bool) {
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

    /// Writes whatever the offer still owes into the clipboard while this window still owns it, so destroying the window cannot lose the text. `Kept` when the text is on the clipboard, `Taken` when someone else owns the clipboard now (a newer copy: nothing of ours is left to keep, and it must not be overwritten), Err when the clipboard stayed busy or the write failed.
    pub fn materialize(&self) -> Result<Materialized, String> {
        if self.lost() {
            return Ok(Materialized::Taken);
        }
        open_within(Some(self.hwnd), Duration::from_millis(500))?;
        let ours = unsafe { GetClipboardOwner() }.ok() == Some(self.hwnd);
        if ours {
            render_pending();
        }
        let _ = unsafe { windows::Win32::System::DataExchange::CloseClipboard() };
        match (ours, OFFER.with(|o| o.borrow().pending_text)) {
            (false, _) => Ok(Materialized::Taken),
            (true, true) => Err("Could not write the text to the clipboard.".into()),
            (true, false) => Ok(Materialized::Kept),
        }
    }
}

/// What `materialize` found.
#[derive(Debug, PartialEq, Eq)]
pub enum Materialized {
    /// The text is on the clipboard.
    Kept,
    /// Somebody else owns the clipboard now.
    Taken,
}

impl Drop for Owner {
    /// Destroys the window. Safety: the window was made on this thread and is destroyed once.
    fn drop(&mut self) {
        unsafe { DestroyWindow(self.hwnd).ok() };
    }
}

/// Registration of an owner window for clipboard-change messages; removed on drop, so nothing stays registered when the transaction ends.
pub struct Listening {
    hwnd: HWND,
}

impl Owner {
    /// Starts hearing clipboard changes on this window (`WM_CLIPBOARDUPDATE`), counted by `updates`. Err when Windows refuses. Safety: the window belongs to this thread and the registration is removed by `Listening`'s drop.
    pub fn listen(&self) -> Result<Listening, String> {
        OFFER.with(|o| o.borrow_mut().updates = 0);
        unsafe { AddClipboardFormatListener(self.hwnd) }.map_err(|e| e.to_string())?;
        Ok(Listening { hwnd: self.hwnd })
    }

    /// Clipboard changes heard since `listen`.
    pub fn updates(&self) -> u32 {
        OFFER.with(|o| o.borrow().updates)
    }

    /// Serves the messages already waiting, without blocking, so updates that arrived are counted.
    pub fn drain(&self) {
        let mut msg = MSG::default();
        unsafe {
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                DispatchMessageW(&msg);
            }
        }
    }
}

impl Drop for Listening {
    /// Removes the registration. Safety: the window outlives the registration (the owner drops after it).
    fn drop(&mut self) {
        let _ = unsafe { RemoveClipboardFormatListener(self.hwnd) };
    }
}
