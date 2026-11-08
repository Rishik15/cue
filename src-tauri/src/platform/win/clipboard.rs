//! File: clipboard.rs
//! Purpose: Win32 clipboard primitives shared by the paste, selection and borrow paths: guarded open, bounded reads, format writes, plain text left for the user (kept out of history).
//! Contents: with_clipboard — open with retries; set_data — write one format; set_plain_text — leave text for the user to paste by hand; exclude_from_history — keep Cue's entry out of history and sync;
//! read_format_within / TooLarge — read one format only if it fits; sequence — detect a clipboard change; unicode_text — the clipboard text; look_within / Looked — one look at a fresh copy (sequence proof, text and HTML read in the same open, bounded).

use std::time::Duration;

use super::cf_html;

use windows::core::w;
use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL, HWND};
use windows::Win32::System::DataExchange::*;
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE};
use windows::Win32::System::Ole::CF_UNICODETEXT;

/// The most text read from a fresh selection copy: 1 MiB, far above the 200,000 characters any action works on, so a program that publishes gigabytes on Ctrl+C cannot make Cue allocate them.
pub const TEXT_BYTES: usize = 1 << 20;
/// The most Unicode text any other reader takes from the clipboard (auto paste, placeholders): 32 MiB, 16 million characters, so a program that publishes gigabytes cannot make Cue allocate them. A larger clipboard text counts as no text.
pub const ANY_TEXT_BYTES: usize = 32 << 20;
/// The most HTML read from a fresh selection copy: markup is bigger than the text it carries.
pub const HTML_BYTES: usize = 4 << 20;

/// A clipboard block larger than the room given for it; nothing was copied.
pub struct TooLarge;

/// Opens the clipboard (up to ten tries 10 ms apart), runs `f`, closes it. Err when it stayed busy or `f` failed.
pub fn with_clipboard<T>(owner: Option<HWND>, f: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    for _ in 0..10 {
        if unsafe { OpenClipboard(owner) }.is_ok() {
            let r = f();
            let _ = unsafe { CloseClipboard() };
            return r;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    Err("The clipboard is busy.".into())
}

/// Writes one memory-backed format. The clipboard must be open. On success the clipboard owns the memory; on failure it is still ours and is freed here.
pub fn set_data(format: u32, bytes: &[u8]) -> Result<(), String> {
    unsafe {
        let h = GlobalAlloc(GMEM_MOVEABLE, bytes.len()).map_err(|e| e.to_string())?;
        let p = GlobalLock(h) as *mut u8;
        if p.is_null() {
            let _ = GlobalFree(Some(h));
            return Err("Could not lock clipboard memory.".into());
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), p, bytes.len());
        let _ = GlobalUnlock(h);
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

/// Puts `text` on the clipboard for the user to paste by hand, as an entry history ignores (it is not a copy by the user). Hidden characters are stripped first: text that Cue puts on the clipboard never carries them.
pub fn set_plain_text(text: &str) -> Result<(), String> {
    let text = crate::sanitize::strip_hidden(text);
    with_clipboard(None, || {
        unsafe { EmptyClipboard() }.map_err(|e| e.to_string())?;
        set_data(CF_UNICODETEXT.0 as u32, &utf16_bytes(&text))?;
        exclude_from_history()
    })
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
    /// The Unicode text, None when there is none.
    pub text: Option<String>,
    /// The copied HTML fragment, None when there is none or it is larger than `html_room`.
    pub html: Option<String>,
}

/// Opens the clipboard once and reads the text, the HTML and then the sequence number in that one open, so the three belong together and no other program can change the clipboard in between. Ok(None) when the clipboard stayed busy; Err when the text is larger than `text_room` bytes (checked before anything is copied).
pub fn look_within(text_room: usize, html_room: usize) -> Result<Option<Looked>, TooLarge> {
    let format = unsafe { RegisterClipboardFormatW(w!("HTML Format")) };
    let opened = with_clipboard(None, || {
        let text = read_format_within(CF_UNICODETEXT.0 as u32, text_room);
        let html = read_format_within(format, html_room).ok().flatten();
        Ok((sequence(), text, html))
    });
    let Ok((sequence, text, html)) = opened else { return Ok(None) };
    Ok(Some(Looked { sequence, text: text?.map(|b| utf16_text(&b)), html: html.and_then(|b| cf_html::fragment(&b)) }))
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

/// Tells Windows clipboard history, cloud sync and clipboard managers to leave what Cue just put on the clipboard alone. The clipboard must be open.
pub fn exclude_from_history() -> Result<(), String> {
    for name in [w!("CanIncludeInClipboardHistory"), w!("CanUploadToCloudClipboard"), w!("ExcludeClipboardContentFromMonitorProcessing")] {
        set_data(unsafe { RegisterClipboardFormatW(name) }, &0u32.to_le_bytes())?;
    }
    Ok(())
}
