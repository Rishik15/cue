//! File: clipboard.rs
//! Purpose: Win32 clipboard primitives shared by the paste, selection and borrow paths: guarded open, bounded reads, format writes, plain text left for the user (kept out of history).
//! Contents: open_within / with_clipboard / with_clipboard_within — open with a growing pause until a deadline; html_format — the id of "HTML Format"; set_data — write one format; set_plain_text — leave text for the user to paste by hand;
//! read_format_within / TooLarge — read one format only if it fits; sequence — detect a clipboard change; unicode_text — the clipboard text; look_within / Looked — one look at a fresh copy (sequence proof, text and HTML read in the same open, bounded).

use std::time::{Duration, Instant};

use super::cf_html;
use super::clipboard_tx::staged::alloc;

use windows::core::w;
use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL, HWND};
use windows::Win32::System::DataExchange::*;
use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
use windows::Win32::System::Ole::CF_UNICODETEXT;

/// The most text read from a fresh selection copy: 1 MiB, far above the 200,000 characters any action works on, so a program that publishes gigabytes on Ctrl+C cannot make Cue allocate them.
pub const TEXT_BYTES: usize = 1 << 20;
/// The most Unicode text any other reader takes from the clipboard (auto paste, placeholders): 32 MiB, 16 million characters, so a program that publishes gigabytes cannot make Cue allocate them. A larger clipboard text counts as no text.
pub const ANY_TEXT_BYTES: usize = 32 << 20;
/// The most HTML read from a fresh selection copy: markup is bigger than the text it carries.
pub const HTML_BYTES: usize = 4 << 20;

/// The registered clipboard format id of "HTML Format". Safety: registering a name returns the same id every time; the name is a static wide string.
pub fn html_format() -> u32 {
    unsafe { RegisterClipboardFormatW(w!("HTML Format")) }
}

/// A clipboard block larger than the room given for it; nothing was copied.
pub struct TooLarge;

/// Opens the clipboard, retrying with a growing pause (5 ms, doubling to 80 ms) until `within` has passed. The caller closes it. Err when another program kept it open the whole time.
pub fn open_within(owner: Option<HWND>, within: Duration) -> Result<(), String> {
    let deadline = Instant::now() + within;
    let mut pause = Duration::from_millis(5);
    loop {
        if unsafe { OpenClipboard(owner) }.is_ok() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err("The clipboard is busy.".into());
        }
        std::thread::sleep(pause.min(deadline.saturating_duration_since(Instant::now())));
        pause = (pause * 2).min(Duration::from_millis(80));
    }
}

/// Opens the clipboard (waiting up to 100 ms for others), runs `f`, closes it. Err when it stayed busy or `f` failed.
pub fn with_clipboard<T>(owner: Option<HWND>, f: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    with_clipboard_within(owner, Duration::from_millis(100), f)
}

/// `with_clipboard` with a wait of `within`.
pub fn with_clipboard_within<T>(owner: Option<HWND>, within: Duration, f: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    open_within(owner, within)?;
    let r = f();
    let _ = unsafe { CloseClipboard() };
    r
}

/// Writes one memory-backed format. The clipboard must be open. On success the clipboard owns the memory; on failure it is still ours and is freed here.
pub fn set_data(format: u32, bytes: &[u8]) -> Result<(), String> {
    let h = alloc(bytes)?;
    unsafe {
        SetClipboardData(format, Some(HANDLE(h.0))).map(|_| ()).map_err(|e| {
            let _ = GlobalFree(Some(h));
            e.to_string()
        })
    }
}

/// The text as little-endian UTF-16 with a terminating zero, the layout of `CF_UNICODETEXT`.
pub fn utf16_bytes(text: &str) -> Vec<u8> {
    text.encode_utf16().chain(std::iter::once(0)).flat_map(u16::to_le_bytes).collect()
}

/// Puts `text` on the clipboard for the user to paste by hand, as an entry history ignores (it is not a copy by the user). Goes through the coordinator, so it never overlaps another Cue clipboard operation; Err when that stays busy for 5 s or the write fails.
pub fn set_plain_text(text: &str) -> Result<(), String> {
    super::clipboard_tx::enter()?.copy_text(text)
}

/// Changes whenever any program writes the clipboard, so a copy can be detected without reading it.
pub fn sequence() -> u32 {
    unsafe { GetClipboardSequenceNumber() }
}

/// The clipboard's Unicode text up to `ANY_TEXT_BYTES`; None when there is none, it is larger, or the clipboard is busy.
pub fn unicode_text() -> Option<String> {
    with_clipboard(None, || Ok(read_format_within(CF_UNICODETEXT.0 as u32, ANY_TEXT_BYTES))).ok()?.ok()?.map(|b| utf16_text(&b))
}

/// Little-endian UTF-16 bytes up to the first zero as a string (invalid units become U+FFFD).
fn utf16_text(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes.as_chunks::<2>().0.iter().map(|b| u16::from_le_bytes(*b)).take_while(|&u| u != 0).collect();
    String::from_utf16_lossy(&units)
}

/// What one look at the clipboard right after a copy found.
pub struct Looked {
    /// The clipboard sequence number after the reads, while the clipboard was still open: a program that renders its data late changes it during the reads, and nobody else can write while it is open, so any later change is a newer copy and this value is the proof for a restore.
    pub sequence: u32,
    /// The process that owns the clipboard, read in the same open; 0 when it has no owner window.
    pub owner_pid: u32,
    /// The Unicode text, None when there is none (or it was too large).
    pub text: Option<String>,
    /// The Unicode text was larger than the room given for it; nothing of it was copied.
    pub too_large: bool,
    /// The copied HTML fragment, None when there is none or it is larger than `html_room`.
    pub html: Option<String>,
}

/// Opens the clipboard once (waiting up to 200 ms with a growing pause) and reads the text, the HTML, the owner and then the sequence number in that one open, so they belong together and no other program can change the clipboard in between. Ok(None) when the clipboard stayed busy. A text larger than `text_room` bytes is not copied (checked before anything is), and the look says so.
pub fn look_within(text_room: usize, html_room: usize) -> Option<Looked> {
    let format = html_format();
    with_clipboard_within(None, Duration::from_millis(200), || {
        let text = read_format_within(CF_UNICODETEXT.0 as u32, text_room);
        let html = read_format_within(format, html_room).ok().flatten();
        let owner_pid = unsafe { GetClipboardOwner() }.map_or(0, |owner| {
            let mut pid = 0;
            unsafe { windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(owner, Some(&mut pid)) };
            pid
        });
        let too_large = text.is_err();
        Ok(Looked {
            sequence: sequence(),
            owner_pid,
            text: text.ok().flatten().map(|b| utf16_text(&b)),
            too_large,
            html: html.and_then(|b| cf_html::fragment(&b)),
        })
    })
    .ok()
}

/// The bytes of one format, with the clipboard open. Err for a block larger than `room`, checked before anything is copied (a huge image must not be allocated just to be refused).
pub fn read_format_within(format: u32, room: usize) -> Result<Option<Vec<u8>>, TooLarge> {
    unsafe {
        let Ok(data) = GetClipboardData(format) else { return Ok(None) };
        let h = HGLOBAL(data.0);
        let len = GlobalSize(h);
        if len > room {
            return Err(TooLarge);
        }
        let p = GlobalLock(h) as *const u8;
        if p.is_null() {
            return Ok(None);
        }
        let bytes = (len > 0).then(|| std::slice::from_raw_parts(p, len).to_vec());
        let _ = GlobalUnlock(h);
        Ok(bytes)
    }
}

/// The registered clipboard format that marks an entry as written by Cue itself (its content is Cue's process id). Safety: registering a name returns the same id every time; the name is a static wide string.
pub fn cue_marker() -> u32 {
    unsafe { RegisterClipboardFormatW(w!("CueClipboardOrigin")) }
}

/// The Unicode text of the open clipboard up to `room` bytes; None when there is none or it is larger.
pub fn text_in_open(room: usize) -> Option<String> {
    read_format_within(CF_UNICODETEXT.0 as u32, room).ok().flatten().map(|b| utf16_text(&b))
}
