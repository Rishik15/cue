//! File: snapshot.rs
//! Purpose: Take the clipboard as it is, all of it or none of it, to put back when a borrow ends. Admission is all-or-refuse and typed: a clipboard Cue cannot hold losslessly is refused with the reason, never half-kept.
//! Contents: Snapshot — the formats and the sequence number they were read at (take, read_open, stage); Refused — why a clipboard was not admitted (TooBig, Busy, Unreadable, Unsupported); Kind / classify — what to do with one format.
//! Memory formats are copied byte for byte (private and registered ones too, empty blocks included); enhanced metafiles and metafile pictures are copied through their bits; formats Windows rebuilds from another one (text variants from Unicode, a bitmap and palette from a DIB) are skipped; a bitmap with no DIB behind it and the owner-display format are refused.
//! shortcut: a clipboard of 256 MiB or more in total is refused as TooBig (the DIB and DIBV5 of one image both count), raise the budget when an image of that size must survive a selection copy.

use std::mem::size_of;
use std::time::Duration;

use windows::Win32::Foundation::HGLOBAL;
use windows::Win32::Graphics::Gdi::{GetEnhMetaFileBits, GetMetaFileBitsEx, HENHMETAFILE, HMETAFILE};
use windows::Win32::System::DataExchange::{EnumClipboardFormats, GetClipboardData, METAFILEPICT};
use windows::Win32::System::Memory::{GlobalFlags, GlobalLock, GlobalSize, GlobalUnlock};

use super::super::clipboard::open_within;
use super::staged::Staged;

/// The most one snapshot holds, in total: enough for an 8K screenshot.
const BUDGET: usize = 256 << 20;
/// How long taking a snapshot may wait for another program to close the clipboard.
const OPEN_WITHIN: Duration = Duration::from_millis(300);
/// `GlobalFlags` result for a handle that is not a global memory block.
const INVALID_HANDLE: u32 = 0x8000;

/// Why a clipboard was not admitted; nothing was kept.
#[derive(Debug, PartialEq, Eq)]
pub enum Refused {
    /// The formats together are over the budget.
    TooBig,
    /// Another program kept the clipboard open for the whole wait.
    Busy,
    /// A format is listed but could not be read (its owner failed to render it, or it is not memory).
    Unreadable(u32),
    /// A format that cannot be copied faithfully.
    Unsupported(u32),
}

impl Refused {
    /// A sentence for the user saying why the clipboard cannot be borrowed.
    pub fn reason(&self) -> String {
        match self {
            Refused::TooBig => {
                "Your clipboard holds a very large image or file, so Cue cannot borrow it. Clear the clipboard and try again.".into()
            }
            Refused::Busy => "Another program is using the clipboard, so Cue cannot borrow it. Try again in a moment.".into(),
            Refused::Unreadable(f) => format!("Your clipboard holds data Cue cannot read (format {f}), so it cannot be borrowed."),
            Refused::Unsupported(f) => {
                format!("Your clipboard holds data Cue cannot copy faithfully (format {f}), so it cannot be borrowed.")
            }
        }
    }
}

/// What to do with one listed format.
#[derive(Debug, PartialEq, Eq)]
pub enum Kind {
    Skip,
    Memory,
    Emf,
    Pict,
    Refuse,
}

/// What to do with `format`, given every format that is listed (Windows lists the ones it can rebuild too).
pub fn classify(format: u32, all: &[u32]) -> Kind {
    let has = |f: u32| all.contains(&f);
    match format {
        1 | 7 if has(13) => Kind::Skip,
        2 | 9 if has(8) || has(17) => Kind::Skip,
        2 | 9 | 0x80 => Kind::Refuse,
        3 => Kind::Pict,
        14 => Kind::Emf,
        0x81..=0x8F => Kind::Skip,
        _ => Kind::Memory,
    }
}

/// One format as it was read.
enum Item {
    Bytes(u32, Vec<u8>),
    Emf(Vec<u8>),
    Pict { mm: i32, x: i32, y: i32, bits: Vec<u8> },
}

/// The clipboard at one moment, with the sequence number it had (read in the same open, so nothing can fall between the two).
pub struct Snapshot {
    items: Vec<Item>,
    sequence: u32,
}

impl Snapshot {
    /// Opens the clipboard (waiting a little for others), reads it whole and closes it. Refused with the reason when it cannot be kept completely.
    pub fn take() -> Result<Snapshot, Refused> {
        open_within(None, OPEN_WITHIN).map_err(|_| Refused::Busy)?;
        let taken = Snapshot::read_open();
        let _ = unsafe { windows::Win32::System::DataExchange::CloseClipboard() };
        taken
    }

    /// Reads every format of the open clipboard.
    pub fn read_open() -> Result<Snapshot, Refused> {
        let mut all = Vec::new();
        let mut format = unsafe { EnumClipboardFormats(0) };
        while format != 0 {
            all.push(format);
            format = unsafe { EnumClipboardFormats(format) };
        }
        let (mut items, mut total) = (Vec::new(), 0usize);
        for &format in &all {
            let item = match classify(format, &all) {
                Kind::Skip => continue,
                Kind::Refuse => return Err(Refused::Unsupported(format)),
                Kind::Memory => read_bytes(format, BUDGET - total)?,
                Kind::Emf => read_emf(BUDGET - total)?,
                Kind::Pict => read_pict(BUDGET - total)?,
            };
            total += item.len();
            items.push(item);
        }
        Ok(Snapshot { items, sequence: super::super::clipboard::sequence() })
    }

    /// The clipboard's sequence number at the moment it was read.
    pub fn sequence(&self) -> u32 {
        self.sequence
    }

    /// Whether the clipboard held nothing that is kept.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Prepares every format for writing back, allocating all of it before the clipboard is touched. Err when memory ran out.
    pub fn stage(&self) -> Result<Staged, String> {
        let mut staged = Staged::new();
        for item in &self.items {
            match item {
                Item::Bytes(format, bytes) => staged.add_bytes(*format, bytes)?,
                Item::Emf(bits) => staged.add_emf(bits)?,
                Item::Pict { mm, x, y, bits } => staged.add_pict(*mm, *x, *y, bits)?,
            }
        }
        Ok(staged)
    }
}

impl Item {
    /// The bytes this item holds, for the budget.
    fn len(&self) -> usize {
        match self {
            Item::Bytes(_, b) | Item::Emf(b) => b.len(),
            Item::Pict { bits, .. } => bits.len(),
        }
    }
}

/// One memory-backed format, with the clipboard open. Refused TooBig before anything is copied when it is over `room`. Safety: the block is locked only while it is copied.
fn read_bytes(format: u32, room: usize) -> Result<Item, Refused> {
    unsafe {
        let data = GetClipboardData(format).map_err(|_| Refused::Unreadable(format))?;
        let h = HGLOBAL(data.0);
        let len = GlobalSize(h);
        if len > room {
            return Err(Refused::TooBig);
        }
        if len == 0 {
            return if GlobalFlags(h) == INVALID_HANDLE { Err(Refused::Unreadable(format)) } else { Ok(Item::Bytes(format, Vec::new())) };
        }
        let p = GlobalLock(h) as *const u8;
        if p.is_null() {
            return Err(Refused::Unreadable(format));
        }
        let bytes = std::slice::from_raw_parts(p, len).to_vec();
        let _ = GlobalUnlock(h);
        Ok(Item::Bytes(format, bytes))
    }
}

/// The bits of the clipboard's enhanced metafile. Safety: the handle belongs to the clipboard and is only read.
fn read_emf(room: usize) -> Result<Item, Refused> {
    unsafe {
        let data = GetClipboardData(14).map_err(|_| Refused::Unreadable(14))?;
        let emf = HENHMETAFILE(data.0);
        let len = GetEnhMetaFileBits(emf, None) as usize;
        if len == 0 {
            return Err(Refused::Unreadable(14));
        }
        if len > room {
            return Err(Refused::TooBig);
        }
        let mut bits = vec![0u8; len];
        if GetEnhMetaFileBits(emf, Some(&mut bits)) as usize != len {
            return Err(Refused::Unreadable(14));
        }
        Ok(Item::Emf(bits))
    }
}

/// The picture block and bits of the clipboard's classic metafile. Safety: the picture block is locked while its fields are read; the metafile handle belongs to the clipboard and is only read.
fn read_pict(room: usize) -> Result<Item, Refused> {
    unsafe {
        let data = GetClipboardData(3).map_err(|_| Refused::Unreadable(3))?;
        let h = HGLOBAL(data.0);
        if GlobalSize(h) < size_of::<METAFILEPICT>() {
            return Err(Refused::Unreadable(3));
        }
        let p = GlobalLock(h) as *const METAFILEPICT;
        if p.is_null() {
            return Err(Refused::Unreadable(3));
        }
        let (mm, x, y, meta): (i32, i32, i32, HMETAFILE) = ((*p).mm, (*p).xExt, (*p).yExt, (*p).hMF);
        let _ = GlobalUnlock(h);
        let len = GetMetaFileBitsEx(meta, 0, None) as usize;
        if len == 0 {
            return Err(Refused::Unreadable(3));
        }
        if len > room {
            return Err(Refused::TooBig);
        }
        let mut bits = vec![0u8; len];
        if GetMetaFileBitsEx(meta, len as u32, Some(bits.as_mut_ptr() as *mut _)) as usize != len {
            return Err(Refused::Unreadable(3));
        }
        Ok(Item::Pict { mm, x, y, bits })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Text variants and a bitmap with a DIB behind them are rebuilt by Windows and skipped; without their source they are kept or refused.
    #[test]
    fn derived_formats_are_skipped_only_when_their_source_is_listed() {
        assert_eq!(classify(1, &[1, 13]), Kind::Skip);
        assert_eq!(classify(7, &[7, 13]), Kind::Skip);
        assert_eq!(classify(1, &[1]), Kind::Memory);
        assert_eq!(classify(2, &[2, 8]), Kind::Skip);
        assert_eq!(classify(9, &[2, 9, 17]), Kind::Skip);
        assert_eq!(classify(2, &[2]), Kind::Refuse);
    }

    /// Both metafile kinds, the private range and the GDI-object range are copied; the owner-display format is refused.
    #[test]
    fn metafiles_and_private_ranges_are_kept_and_owner_display_is_refused() {
        assert_eq!(classify(3, &[3]), Kind::Pict);
        assert_eq!(classify(14, &[14]), Kind::Emf);
        assert_eq!(classify(0x305, &[0x305]), Kind::Memory);
        assert_eq!(classify(0x203, &[0x203]), Kind::Memory);
        assert_eq!(classify(0xC123, &[0xC123]), Kind::Memory);
        assert_eq!(classify(0x80, &[0x80]), Kind::Refuse);
        assert_eq!(classify(0x82, &[0x82, 13]), Kind::Skip);
    }

    /// Every refusal explains itself to the user.
    #[test]
    fn every_refusal_has_a_reason() {
        for refused in [Refused::TooBig, Refused::Busy, Refused::Unreadable(7), Refused::Unsupported(2)] {
            assert!(!refused.reason().is_empty());
        }
    }
}
