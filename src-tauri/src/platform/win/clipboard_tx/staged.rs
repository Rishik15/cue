//! File: staged.rs
//! Purpose: Clipboard contents prepared before the clipboard is touched. Every memory block and metafile is allocated first, so an allocation failure leaves the old clipboard alone, and what the clipboard did not take is freed here.
//! Contents: Staged — the prepared formats (add_bytes, add_emf, add_pict, add_delayed, add_exclusions) and write_all, which hands them to the open clipboard in order; alloc — one global memory block.

use std::mem::size_of;

use windows::core::w;
use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL};
use windows::Win32::Graphics::Gdi::{DeleteEnhMetaFile, DeleteMetaFile, SetEnhMetaFileBits, SetMetaFileBitsEx, HENHMETAFILE};
use windows::Win32::System::DataExchange::{IsClipboardFormatAvailable, RegisterClipboardFormatW, SetClipboardData, METAFILEPICT};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};

/// The clipboard format number of an enhanced metafile.
const CF_ENHMETAFILE: u32 = 14;
/// The clipboard format number of a classic metafile picture.
const CF_METAFILEPICT: u32 = 3;

/// One prepared format and what still has to be freed if the clipboard never takes it.
enum Slot {
    Global(HGLOBAL),
    Emf(HENHMETAFILE),
    Pict(HGLOBAL),
    Delayed,
    Given,
}

/// The formats to write, in order.
pub struct Staged(Vec<(u32, Slot)>);

/// A new global memory block holding `bytes` (an empty block for no bytes). Err when Windows has no memory. Safety: the block is locked only while it is filled.
pub fn alloc(bytes: &[u8]) -> Result<HGLOBAL, String> {
    unsafe {
        let h = GlobalAlloc(GMEM_MOVEABLE, bytes.len()).map_err(|e| e.to_string())?;
        if bytes.is_empty() {
            return Ok(h);
        }
        let p = GlobalLock(h) as *mut u8;
        if p.is_null() {
            let _ = GlobalFree(Some(h));
            return Err("Could not lock clipboard memory.".into());
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), p, bytes.len());
        let _ = GlobalUnlock(h);
        Ok(h)
    }
}

impl Staged {
    /// Nothing prepared yet.
    pub fn new() -> Staged {
        Staged(Vec::new())
    }

    /// Prepares one memory-backed format. Err when memory ran out.
    pub fn add_bytes(&mut self, format: u32, bytes: &[u8]) -> Result<(), String> {
        self.0.push((format, Slot::Global(alloc(bytes)?)));
        Ok(())
    }

    /// Prepares an enhanced metafile from its bits. Err when Windows rejects them.
    pub fn add_emf(&mut self, bits: &[u8]) -> Result<(), String> {
        let emf = unsafe { SetEnhMetaFileBits(bits) };
        if emf.is_invalid() {
            return Err("Could not rebuild an enhanced metafile.".into());
        }
        self.0.push((CF_ENHMETAFILE, Slot::Emf(emf)));
        Ok(())
    }

    /// Prepares a classic metafile picture: its mapping mode, extents and bits. Err when Windows rejects them. Safety: the picture block is filled before anyone else can see it.
    pub fn add_pict(&mut self, mm: i32, x: i32, y: i32, bits: &[u8]) -> Result<(), String> {
        unsafe {
            let meta = SetMetaFileBitsEx(bits);
            if meta.is_invalid() {
                return Err("Could not rebuild a metafile picture.".into());
            }
            let block = match alloc(&vec![0; size_of::<METAFILEPICT>()]) {
                Ok(block) => block,
                Err(e) => {
                    let _ = DeleteMetaFile(meta);
                    return Err(e);
                }
            };
            std::ptr::write(GlobalLock(block) as *mut METAFILEPICT, METAFILEPICT { mm, xExt: x, yExt: y, hMF: meta });
            let _ = GlobalUnlock(block);
            self.0.push((CF_METAFILEPICT, Slot::Pict(block)));
        }
        Ok(())
    }

    /// Prepares a delayed-render entry: the owner window supplies the data when someone asks.
    pub fn add_delayed(&mut self, format: u32) {
        self.0.push((format, Slot::Delayed));
    }

    /// Prepares the three markers that keep an entry out of Windows clipboard history, cloud sync and clipboard managers.
    pub fn add_exclusions(&mut self) -> Result<(), String> {
        for name in
            [w!("CanIncludeInClipboardHistory"), w!("CanUploadToCloudClipboard"), w!("ExcludeClipboardContentFromMonitorProcessing")]
        {
            self.add_bytes(unsafe { RegisterClipboardFormatW(name) }, &0u32.to_le_bytes())?;
        }
        Ok(())
    }

    /// Hands every prepared format to the clipboard, which must be open and emptied by the caller. What it takes belongs to it; Err names the first it refused, and the rest is freed on drop. Safety: each handle is given once and never touched after the clipboard accepts it.
    pub fn write_all(&mut self) -> Result<(), String> {
        for (format, slot) in &mut self.0 {
            let handle = match slot {
                Slot::Global(h) | Slot::Pict(h) => Some(HANDLE(h.0)),
                Slot::Emf(e) => Some(HANDLE(e.0)),
                Slot::Delayed => None,
                Slot::Given => continue,
            };
            let taken = unsafe { SetClipboardData(*format, handle) };
            let delayed = matches!(slot, Slot::Delayed);
            let accepted = if delayed { unsafe { IsClipboardFormatAvailable(*format) }.is_ok() } else { taken.is_ok() };
            if !accepted {
                return Err("The clipboard refused the data.".into());
            }
            *slot = Slot::Given;
        }
        Ok(())
    }
}

impl Drop for Staged {
    /// Frees what the clipboard never took. Safety: only handles still owned here are released.
    fn drop(&mut self) {
        for (_, slot) in &self.0 {
            unsafe {
                match slot {
                    Slot::Global(h) => {
                        let _ = GlobalFree(Some(*h));
                    }
                    Slot::Emf(e) => {
                        let _ = DeleteEnhMetaFile(Some(*e));
                    }
                    Slot::Pict(h) => {
                        let p = GlobalLock(*h) as *const METAFILEPICT;
                        if !p.is_null() {
                            let _ = DeleteMetaFile((*p).hMF);
                            let _ = GlobalUnlock(*h);
                        }
                        let _ = GlobalFree(Some(*h));
                    }
                    Slot::Delayed | Slot::Given => {}
                }
            }
        }
    }
}
