//! Purpose: Win32 clipboard primitives shared by the paste path: guarded open, whole-clipboard snapshot and restore,
//! plain text write.
//! Contents: with_clipboard — open with retries; set_data — write one format; Snapshot / snapshot / restore — keep every
//! memory-backed format (text, images, files, rich text) so a paste never destroys what the user had copied;
//! set_plain_text — leave text for the user to paste by hand.

use std::time::Duration;

use windows::Win32::Foundation::{HANDLE, HGLOBAL, HWND};
use windows::Win32::System::DataExchange::*;
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE};
use windows::Win32::System::Ole::{CF_OEMTEXT, CF_TEXT, CF_UNICODETEXT};

const MAX_SNAPSHOT_BYTES: usize = 64 << 20;

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
            return Err("Could not lock clipboard memory.".into());
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), p, bytes.len());
        let _ = GlobalUnlock(h);
        SetClipboardData(format, Some(HANDLE(h.0))).map(|_| ()).map_err(|e| e.to_string())
    }
}

pub fn utf16_bytes(text: &str) -> Vec<u8> {
    text.encode_utf16().chain(std::iter::once(0)).flat_map(u16::to_le_bytes).collect()
}

pub fn set_plain_text(text: &str) -> Result<(), String> {
    with_clipboard(None, || {
        unsafe { EmptyClipboard() }.map_err(|e| e.to_string())?;
        set_data(CF_UNICODETEXT.0 as u32, &utf16_bytes(text))
    })
}

/// Every memory-backed format, or None when the clipboard was too large to hold (then it is left untouched).
pub struct Snapshot(Option<Vec<(u32, Vec<u8>)>>);

/// Handle-backed (GDI) formats are skipped: Windows rebuilds bitmaps from the DIB, and text variants from Unicode.
fn restorable(f: u32) -> bool {
    !matches!(f, 2 | 3 | 9 | 14 | 0x80..=0x8F | 0x300..=0x3FF) && f != CF_TEXT.0 as u32 && f != CF_OEMTEXT.0 as u32
}

fn read_format(format: u32) -> Option<Vec<u8>> {
    unsafe {
        let h = HGLOBAL(GetClipboardData(format).ok()?.0);
        let len = GlobalSize(h);
        let p = GlobalLock(h) as *const u8;
        if p.is_null() || len == 0 {
            return None;
        }
        let bytes = std::slice::from_raw_parts(p, len).to_vec();
        let _ = GlobalUnlock(h);
        Some(bytes)
    }
}

pub fn snapshot() -> Snapshot {
    let items = with_clipboard(None, || {
        let (mut items, mut total) = (Vec::new(), 0usize);
        let mut format = unsafe { EnumClipboardFormats(0) };
        while format != 0 {
            if let Some(bytes) = restorable(format).then(|| read_format(format)).flatten() {
                total += bytes.len();
                if total > MAX_SNAPSHOT_BYTES {
                    return Err("Clipboard too large to restore.".into());
                }
                items.push((format, bytes));
            }
            format = unsafe { EnumClipboardFormats(format) };
        }
        Ok(items)
    });
    Snapshot(items.ok())
}

pub fn restore(snap: Snapshot) {
    let Some(items) = snap.0 else { return };
    let _ = with_clipboard(None, || {
        unsafe { EmptyClipboard() }.map_err(|e| e.to_string())?;
        items.iter().for_each(|(f, b)| drop(set_data(*f, b)));
        Ok(())
    });
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
        assert!(restorable(CF_UNICODETEXT.0 as u32) && restorable(15) && restorable(8));
        // text, file list, DIB
    }
}
