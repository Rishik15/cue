//! File: mod.rs
//! Purpose: The clipboard transaction coordinator. One gate covers every Cue clipboard write and every injected input: a selection copy, a delivery, a rescue copy and a dictation copy each hold a `Tx`, so none can empty, restore or type over another's. Lock order: this gate first, the input lift second, never reversed.
//! Contents: written_or_refused — an unconditional copy that was refused is an error; enter / Tx — take the gate (5 s, then Busy) and write plain text for the user through a real owner window, rolled back on failure and retried only at the sequence its failed attempt left (copy_text, copy_text_unless_newer — the same only while no newer copy exists); owner — the clipboard owner window and the delayed-render offer; snapshot — all-or-refuse clipboard admission; staged — formats prepared before the clipboard is touched; gate — the lock with a deadline.

pub mod gate;
pub mod origin;
pub mod owner;
pub mod reader;
pub mod snapshot;
pub mod staged;

use std::cell::Cell;
use std::time::{Duration, Instant};

use windows::Win32::System::DataExchange::EmptyClipboard;
use windows::Win32::System::Ole::CF_UNICODETEXT;

pub use owner::Owner;

use super::borrow::Borrow;
use super::clipboard::{sequence, utf16_bytes, with_clipboard_within};
use gate::{Gate, Held};
use snapshot::Snapshot;
use staged::Staged;

/// The one gate of the process.
static GATE: Gate = Gate::new();
/// The longest a caller waits for the gate before it reports Cue busy.
const ENTER_WITHIN: Duration = Duration::from_secs(5);
/// The longest a plain copy waits for another program to let go of the clipboard, over all its attempts.
const COPY_WITHIN: Duration = Duration::from_millis(1500);

/// Proof that the coordinator is held; dropping it frees the next transaction.
pub struct Tx {
    _held: Held,
}

/// Takes the coordinator, waiting at most 5 s. Err with a user-readable line when another Cue clipboard operation kept it that long.
pub fn enter() -> Result<Tx, String> {
    GATE.enter(ENTER_WITHIN)
        .map(|held| Tx { _held: held })
        .map_err(|_| "Cue is busy with another clipboard operation. Try again.".to_string())
}

impl Tx {
    /// Puts `text` on the clipboard for the user to paste by hand, as an entry history ignores. Hidden characters are stripped first. A real owner window opens the clipboard, everything is allocated before it is emptied, and a busy clipboard is retried with a growing pause for up to 1.5 s. Err when it never succeeded; the old clipboard is then untouched or emptied only by an attempt that failed mid-write.
    pub fn copy_text(&self, text: &str) -> Result<(), String> {
        self.copy_text_unless_newer(text, None).and_then(written_or_refused)
    }

    /// `copy_text`, but only while the clipboard is still at sequence number `seen` (checked while it is open, so nobody can copy in between): Ok(true) when the text was written, Ok(false) when the user copied something newer, which is left alone. `None` writes unconditionally.
    pub fn copy_text_unless_newer(&self, text: &str, seen: Option<u32>) -> Result<bool, String> {
        let text = crate::sanitize::strip_hidden(text);
        let owner = Owner::new()?;
        let deadline = Instant::now() + COPY_WITHIN;
        let partial: Cell<Option<u32>> = Cell::new(None);
        loop {
            let attempt = write_once(&owner, &text, deadline, seen, &partial);
            if attempt.is_ok() || Instant::now() >= deadline {
                return attempt;
            }
        }
    }
}

/// The result of an unconditional copy: a write that was refused because a newer copy appeared after a failed attempt is an error, never a success.
fn written_or_refused(written: bool) -> Result<(), String> {
    if written {
        Ok(())
    } else {
        Err("The clipboard changed while Cue was copying, so Cue left it alone.".to_string())
    }
}

/// Whether the clipboard is still where a write may go on: a first attempt needs the caller's sequence `seen` (none: any), a retry needs the sequence `partial` that the failed attempt left, so an intervening copy is never emptied.
fn baseline_holds(seen: Option<u32>, partial: Option<u32>, now: u32) -> bool {
    match partial {
        Some(left) => left == now,
        None => seen.is_none_or(|s| s == now),
    }
}

/// One attempt to replace the clipboard with `text` (Ok(false): the clipboard is no longer where this write may go on, nothing was written), prepared before the clipboard is opened for `owner` (no longer than until `deadline`). The old clipboard is read in the same open and put back when the write fails; the sequence the attempt left is recorded in `partial` for the retry.
fn write_once(owner: &Owner, text: &str, deadline: Instant, seen: Option<u32>, partial: &Cell<Option<u32>>) -> Result<bool, String> {
    let mut staged = Staged::new();
    staged.add_bytes(CF_UNICODETEXT.0 as u32, &utf16_bytes(text))?;
    staged.add_exclusions()?;
    let within = deadline.saturating_duration_since(Instant::now()).max(Duration::from_millis(50));
    with_clipboard_within(Some(owner.hwnd()), within, || {
        if !baseline_holds(seen, partial.get(), sequence()) {
            return Ok(false);
        }
        let old = Snapshot::read_open().map(Borrow::of);
        let written = unsafe { EmptyClipboard() }.map_err(|e| e.to_string()).and_then(|()| staged.write_all());
        if let (Err(_), Ok(old)) = (&written, &old) {
            let _ = old.put_back_in_open();
        }
        partial.set(Some(sequence()));
        written.map(|()| true)
    })
}

#[cfg(test)]
mod tests {
    use super::{baseline_holds, written_or_refused};

    /// An unconditional copy that was refused after a retry sees a newer copy is reported as an error.
    #[test]
    fn a_refused_unconditional_copy_is_an_error() {
        assert!(written_or_refused(true).is_ok());
        assert!(written_or_refused(false).is_err());
    }

    /// A first attempt goes on at the caller's sequence (or anywhere when none was given); a retry only at the one the failed attempt left.
    #[test]
    fn a_retry_never_empties_an_intervening_copy() {
        assert!(baseline_holds(None, None, 9));
        assert!(baseline_holds(Some(5), None, 5));
        assert!(!baseline_holds(Some(5), None, 6));
        assert!(baseline_holds(None, Some(7), 7));
        assert!(!baseline_holds(None, Some(7), 8));
        assert!(!baseline_holds(Some(5), Some(7), 5));
    }
}
