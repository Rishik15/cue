//! Purpose: Win32 clipboard primitives shared by the paste path: guarded open, whole-clipboard snapshot and restore,
//! plain text write (kept out of history).
//! Contents: with_clipboard — open with retries; set_data — write one format; Snapshot / snapshot / restore — keep every
//! memory-backed format (text, images, files, rich text) so a paste never destroys what the user had copied;
//! set_plain_text — leave text for the user to paste by hand; sequence / unicode_text — detect a clipboard change and read its text.

use std::time::{Duration, Instant};

use super::cf_html;

use windows::core::w;
use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL, HWND};
use windows::Win32::System::DataExchange::*;
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE};
use windows::Win32::System::Ole::{CF_OEMTEXT, CF_TEXT, CF_UNICODETEXT};

/// What a selection copy may borrow. A bigger clipboard (a huge image) is not borrowed at all: a copy that could not be undone is never started.
pub const BORROW_BYTES: usize = 64 << 20;
/// What a paste keeps to restore afterwards; dictated text must reach the app even when the clipboard is bigger.
pub const PASTE_BYTES: usize = 64 << 20;
/// How long a restore keeps trying while another program holds the clipboard open.
const RESTORE_WITHIN: Duration = Duration::from_millis(500);

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

/// Writes one memory-backed format. The clipboard must be open.
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
        // On success the clipboard owns the memory; on failure it is still ours to free.
        SetClipboardData(format, Some(HANDLE(h.0))).map(|_| ()).map_err(|e| {
            let _ = GlobalFree(Some(h));
            e.to_string()
        })
    }
}

pub fn utf16_bytes(text: &str) -> Vec<u8> {
    text.encode_utf16().chain(std::iter::once(0)).flat_map(u16::to_le_bytes).collect()
}

pub fn set_plain_text(text: &str) -> Result<(), String> {
    let text = crate::sanitize::strip_hidden(text); // text that Cue puts on the clipboard never carries hidden characters
    with_clipboard(None, || {
        unsafe { EmptyClipboard() }.map_err(|e| e.to_string())?;
        set_data(CF_UNICODETEXT.0 as u32, &utf16_bytes(&text))?;
        exclude_from_history() // text left for a manual paste is not a copy by the user
    })
}

/// Changes whenever any program writes the clipboard, so a copy can be detected without reading it.
pub fn sequence() -> u32 {
    unsafe { GetClipboardSequenceNumber() }
}

pub fn unicode_text() -> Option<String> {
    let bytes = with_clipboard(None, || read_format(CF_UNICODETEXT.0 as u32).ok_or_else(String::new)).ok()?;
    let units: Vec<u16> = bytes.as_chunks::<2>().0.iter().map(|b| u16::from_le_bytes(*b)).take_while(|&u| u != 0).collect();
    Some(String::from_utf16_lossy(&units))
}

/// The HTML a program put on the clipboard ("HTML Format"), reduced to the copied fragment. None when there is none.
pub fn html_fragment() -> Option<String> {
    // SAFETY: registering a clipboard format name returns the same id every time.
    let format = unsafe { RegisterClipboardFormatW(w!("HTML Format")) };
    let bytes = with_clipboard(None, || read_format(format).ok_or_else(String::new)).ok()?;
    cf_html::fragment(&bytes)
}

/// Every memory-backed format of the clipboard at one moment.
pub struct Snapshot(Vec<(u32, Vec<u8>)>);

/// Handle-backed (GDI) formats are skipped: Windows rebuilds bitmaps from the DIB (and CF_DIBV5, 17, from it too, so an image is not held twice), and text variants from Unicode.
fn restorable(f: u32) -> bool {
    !matches!(f, 2 | 3 | 9 | 14 | 17 | 0x80..=0x8F | 0x300..=0x3FF) && f != CF_TEXT.0 as u32 && f != CF_OEMTEXT.0 as u32
}

fn read_format(format: u32) -> Option<Vec<u8>> {
    read_format_within(format, usize::MAX).ok().flatten()
}

/// Err for a block larger than `room`, checked before anything is copied (a huge image must not be allocated just to be refused).
fn read_format_within(format: u32, room: usize) -> Result<Option<Vec<u8>>, ()> {
    unsafe {
        let Ok(data) = GetClipboardData(format) else { return Ok(None) };
        let h = HGLOBAL(data.0);
        let len = GlobalSize(h);
        if len > room {
            return Err(());
        }
        let p = GlobalLock(h) as *const u8;
        if p.is_null() {
            return Ok(None);
        }
        let bytes = (len > 0).then(|| std::slice::from_raw_parts(p, len).to_vec());
        let _ = GlobalUnlock(h); // every successful lock is released, also for an empty block
        Ok(bytes)
    }
}

/// The clipboard as it is now, for putting back after Cue borrows it. Err when it holds more than `cap` bytes (or is busy): the caller must then not borrow it.
pub fn snapshot(cap: usize) -> Result<Snapshot, String> {
    with_clipboard(None, || {
        let (mut items, mut total) = (Vec::new(), 0usize);
        let mut format = unsafe { EnumClipboardFormats(0) };
        while format != 0 {
            if restorable(format) {
                let bytes = read_format_within(format, cap - total)
                    .map_err(|()| "Your clipboard holds a very large image or file, so Cue cannot borrow it to read the selection. Clear the clipboard and try again.".to_string())?;
                total += bytes.as_ref().map_or(0, Vec::len);
                items.extend(bytes.map(|b| (format, b)));
            }
            format = unsafe { EnumClipboardFormats(format) };
        }
        Ok(Snapshot(items))
    })
}

/// Tells Windows clipboard history, cloud sync and clipboard managers to leave what Cue just put on the clipboard alone. The clipboard must be open.
pub fn exclude_from_history() -> Result<(), String> {
    for name in [w!("CanIncludeInClipboardHistory"), w!("CanUploadToCloudClipboard"), w!("ExcludeClipboardContentFromMonitorProcessing")] {
        set_data(unsafe { RegisterClipboardFormatW(name) }, &0u32.to_le_bytes())?;
    }
    Ok(())
}

/// Puts the snapshot back, as an entry history and clipboard managers ignore (it is not a new copy by the user). A busy clipboard is retried for up to `RESTORE_WITHIN`; Err when it never opened.
pub fn restore(snap: Snapshot) -> Result<(), String> {
    let items = snap.0;
    let deadline = Instant::now() + RESTORE_WITHIN;
    let result = loop {
        let attempt = with_clipboard(None, || {
            unsafe { EmptyClipboard() }.map_err(|e| e.to_string())?;
            items.iter().for_each(|(f, b)| drop(set_data(*f, b)));
            if items.is_empty() {
                return Ok(()); // it was empty: it is empty again, with no marker formats left behind
            }
            exclude_from_history()
        });
        if attempt.is_ok() || Instant::now() >= deadline {
            break attempt;
        }
    };
    crate::mem::trim_soon(); // the copies of the clipboard are freed
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_is_nul_terminated_little_endian() {
        assert_eq!(utf16_bytes("a"), [b'a', 0, 0, 0]);
    }

    #[test]
    fn snapshot_skips_handle_and_derived_formats() {
        assert!(!restorable(CF_TEXT.0 as u32) && !restorable(2) && !restorable(0x305));
        assert!(restorable(CF_UNICODETEXT.0 as u32) && restorable(15) && restorable(8)); // text, file list, DIB
        assert!(!restorable(17), "CF_DIBV5 is rebuilt from CF_DIB");
    }
}
