//! File: borrow.rs
//! Purpose: Borrow the clipboard for a moment (a selection copy, a paste) and give it back as one transaction: admitted whole or refused, and put back only while it still holds Cue's own temporary value, so a newer copy by the user is never overwritten.
//! Contents: Ours — the proof that the clipboard is still Cue's; Restored — how finishing ended; Borrow — begin (admit the clipboard, see `clipboard_tx::snapshot`) and finish (conditional restore through a real owner window, retried while the clipboard is busy; when it finally fails the payload is left on the clipboard).

use std::cell::Cell;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::HWND;
use windows::Win32::System::DataExchange::{EmptyClipboard, GetClipboardOwner};

use super::clipboard::{sequence, with_clipboard_within};
use super::clipboard_tx::snapshot::{Refused, Snapshot};
use super::clipboard_tx::staged::Staged;
use super::clipboard_tx::Owner;

/// How long a restore keeps trying while another program holds the clipboard open.
const RESTORE_WITHIN: Duration = Duration::from_millis(500);
/// The longest one restore attempt waits for the clipboard to open.
const OPEN_WITHIN: Duration = Duration::from_millis(100);

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
pub fn newer_than_partial(left: u32, now: u32) -> bool {
    left != now
}

/// The clipboard as it was, to put back when the borrow ends.
pub struct Borrow(Snapshot);

impl Borrow {
    /// Admits the clipboard as it is now. Refused, with the typed reason, when it cannot be kept completely: the caller must then not empty it expecting to restore.
    pub fn begin() -> Result<Borrow, Refused> {
        Snapshot::take().map(Borrow)
    }

    /// A borrow of a snapshot already read (in the same open as the write that follows it).
    pub fn of(snapshot: Snapshot) -> Borrow {
        Borrow(snapshot)
    }

    /// Puts the old clipboard back through `owner` as an entry history ignores, but only if `ours` still holds when the clipboard is first opened for it; otherwise the newer copy stays.
    /// A busy clipboard is retried for up to `RESTORE_WITHIN`. Each attempt that wrote anything leaves the sequence number it ended on, and a retry goes on only while that is still the sequence (the half-restored clipboard is still ours); a newer copy in between is kept. Everything is allocated before each attempt empties the clipboard.
    /// When it never completed, `payload` (UTF-16 text) is written back so the text survives, and Err says so.
    pub fn finish(self, ours: Ours, owner: &Owner, payload: Option<&[u8]>) -> Result<Restored, String> {
        let deadline = Instant::now() + RESTORE_WITHIN;
        let partial: Cell<Option<u32>> = Cell::new(None);
        let result = loop {
            let attempt = self.attempt(&ours, owner, &partial);
            if attempt.is_ok() || Instant::now() >= deadline {
                break attempt;
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        if result == Ok(Restored::NewerCopy) {
            crate::log::note("clipboard: a newer copy was kept");
        }
        if result.is_err() {
            leave_payload(owner, payload, partial.get());
        }
        crate::mem::trim_soon();
        result
    }

    /// Puts the old clipboard back while the caller already has the clipboard open for the owner window (a failed publication): empties it and writes everything. Err when staging or writing fails.
    pub fn put_back_in_open(&self) -> Result<(), String> {
        let mut staged = self.0.stage()?;
        if !self.0.is_empty() {
            staged.add_exclusions()?;
        }
        unsafe { EmptyClipboard() }.map_err(|e| e.to_string())?;
        staged.write_all()
    }

    /// One try: allocate, open for `owner`, check the clipboard is still Cue's, empty it, write everything back.
    fn attempt(&self, ours: &Ours, owner: &Owner, partial: &Cell<Option<u32>>) -> Result<Restored, String> {
        let mut staged = self.0.stage()?;
        if !self.0.is_empty() {
            staged.add_exclusions()?;
        }
        with_clipboard_within(Some(owner.hwnd()), OPEN_WITHIN, || {
            let newer = match partial.get() {
                None => !ours.holds(),
                Some(left) => newer_than_partial(left, sequence()),
            };
            if newer {
                return Ok(Restored::NewerCopy);
            }
            let done = unsafe { EmptyClipboard() }.map_err(|e| e.to_string()).and_then(|()| staged.write_all());
            partial.set(Some(sequence()));
            done.map(|()| Restored::Back)
        })
    }
}

/// After a restore that never completed: if the clipboard is still the half-restored one Cue left (sequence `left`), replaces it with the payload text so the text survives. Best effort; a newer copy is never touched.
fn leave_payload(owner: &Owner, payload: Option<&[u8]>, left: Option<u32>) {
    let (Some(bytes), Some(left)) = (payload, left) else { return };
    let mut staged = Staged::new();
    if staged.add_bytes(windows::Win32::System::Ole::CF_UNICODETEXT.0 as u32, bytes).is_err() || staged.add_exclusions().is_err() {
        return;
    }
    let _ = with_clipboard_within(Some(owner.hwnd()), OPEN_WITHIN, || {
        if newer_than_partial(left, sequence()) {
            return Ok(());
        }
        unsafe { EmptyClipboard() }.map_err(|e| e.to_string())?;
        staged.write_all()
    });
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
