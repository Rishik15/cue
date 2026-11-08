//! File: borrow.rs
//! Purpose: Borrow the clipboard for a moment (a selection copy, a paste) and give it back as one transaction: bounded, and put back only while it still holds Cue's own temporary value, so a newer copy by the user is never overwritten.
//! Contents: Ours — the proof that the clipboard is still Cue's; Restored — how finishing ended; Borrow — begin (take every memory-backed format) and finish (conditional restore, retried while the clipboard is busy).

use std::time::{Duration, Instant};

use windows::Win32::Foundation::HWND;
use windows::Win32::System::DataExchange::{EnumClipboardFormats, GetClipboardOwner};
use windows::Win32::System::Ole::{CF_OEMTEXT, CF_TEXT};

use super::clipboard::{exclude_from_history, read_format_within, sequence, set_data, with_clipboard, TooLarge};

/// The most one borrow holds: 256 MiB, enough for an 8K screenshot, so an image on the clipboard is borrowed, read around and put back without an error. Only a clipboard above this is not borrowed; nothing is allocated for it.
const BORROW_BYTES: usize = 256 << 20;
/// How long a restore keeps trying while another program holds the clipboard open.
const RESTORE_WITHIN: Duration = Duration::from_millis(500);

/// What shows that the clipboard still holds Cue's value, checked while the clipboard is open (nobody can change it meanwhile).
pub enum Ours {
    /// The clipboard sequence number is still this one: nobody has copied since.
    Sequence(u32),
    /// The window that owns the clipboard is still this one.
    Owner(HWND),
}

impl Ours {
    /// Whether the open clipboard still holds Cue's value.
    fn holds(&self) -> bool {
        match self {
            Ours::Sequence(seen) => sequence() == *seen,
            Ours::Owner(window) => unsafe { GetClipboardOwner() }.ok() == Some(*window),
        }
    }
}

/// How a finished borrow ended.
#[derive(Debug, PartialEq, Eq)]
pub enum Restored {
    /// The old clipboard is back (or it was empty and is empty again).
    Back,
    /// The user copied something newer meanwhile; it was left alone.
    NewerCopy,
}

/// Whether the clipboard changed since a failed restore attempt left it at sequence `left`: then the user copied something newer, and finishing the restore would overwrite it.
fn newer_than_partial(left: u32, now: u32) -> bool {
    left != now
}

/// Every memory-backed format of the clipboard at one moment, to put back when the borrow ends.
///
/// shortcut: a memory format that Windows cannot render when read is left out of the restore, refuse the borrow for it once a clipboard that matters is known to fail.
pub struct Borrow(Vec<(u32, Vec<u8>)>);

/// Handle-backed (GDI) formats are skipped: Windows rebuilds bitmaps from the DIB (and CF_DIBV5, 17, from it too, so an image is not held twice), and text variants from Unicode.
fn restorable(f: u32) -> bool {
    !matches!(f, 2 | 3 | 9 | 14 | 17 | 0x80..=0x8F | 0x300..=0x3FF) && f != CF_TEXT.0 as u32 && f != CF_OEMTEXT.0 as u32
}

impl Borrow {
    /// Takes the clipboard as it is now. Err, with a line for the user, when it holds more than `BORROW_BYTES` or is busy: the caller must then not borrow it.
    pub fn begin() -> Result<Borrow, String> {
        with_clipboard(None, || {
            let (mut items, mut total) = (Vec::new(), 0usize);
            let mut format = unsafe { EnumClipboardFormats(0) };
            while format != 0 {
                if restorable(format) {
                    let bytes = read_format_within(format, BORROW_BYTES - total).map_err(|TooLarge| {
                        "Your clipboard holds a very large image or file, so Cue cannot borrow it to read the selection. Clear the clipboard and try again."
                            .to_string()
                    })?;
                    total += bytes.as_ref().map_or(0, Vec::len);
                    items.extend(bytes.map(|b| (format, b)));
                }
                format = unsafe { EnumClipboardFormats(format) };
            }
            Ok(Borrow(items))
        })
    }

    /// Puts the old clipboard back as an entry history and clipboard managers ignore (it is not a new copy by the user), but only if `ours` still holds when the clipboard is first opened for it; otherwise the newer copy stays.
    /// A busy clipboard is retried for up to `RESTORE_WITHIN`. Each attempt that wrote anything leaves the sequence number it ended on, and a retry goes on only while that is still the sequence (the half-restored clipboard is still ours); a newer copy in between is kept. Err when it never completed.
    pub fn finish(self, ours: Ours) -> Result<Restored, String> {
        let deadline = Instant::now() + RESTORE_WITHIN;
        let mut partial: Option<u32> = None;
        let result = loop {
            let attempt = with_clipboard(None, || {
                let newer = match partial {
                    None => !ours.holds(),
                    Some(left) => newer_than_partial(left, sequence()),
                };
                if newer {
                    return Ok(Restored::NewerCopy);
                }
                let done = self.put_back();
                partial = Some(sequence());
                done
            });
            if attempt.is_ok() || Instant::now() >= deadline {
                break attempt;
            }
        };
        if result == Ok(Restored::NewerCopy) {
            crate::log::note("clipboard: a newer copy was kept");
        }
        crate::mem::trim_soon();
        result
    }

    /// Empties the open clipboard and writes every borrowed format back. An empty borrow leaves it empty, with no marker formats behind.
    fn put_back(&self) -> Result<Restored, String> {
        unsafe { windows::Win32::System::DataExchange::EmptyClipboard() }.map_err(|e| e.to_string())?;
        self.0.iter().try_for_each(|(f, b)| set_data(*f, b))?;
        if !self.0.is_empty() {
            exclude_from_history()?;
        }
        Ok(Restored::Back)
    }
}

#[cfg(test)]
mod tests {
    use super::newer_than_partial;

    /// A retry goes on at the sequence the failed attempt left and stops at any other.
    #[test]
    fn a_retry_continues_only_while_the_sequence_is_the_one_the_failed_attempt_left() {
        assert!(!newer_than_partial(41, 41));
        assert!(newer_than_partial(41, 42));
    }
}
