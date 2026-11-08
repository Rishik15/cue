//! File: mod.rs
//! Purpose: The clipboard transaction coordinator. One gate covers every Cue clipboard write and every injected input: a selection copy, a delivery, a rescue copy and a dictation copy each hold a `Tx`, so none can empty, restore or type over another's. Lock order: this gate first, the input lift second, never reversed.
//! Contents: enter / Tx — take the gate (5 s, then Busy) and write plain text for the user through a real owner window (copy_text); owner — the clipboard owner window and the delayed-render offer; snapshot — all-or-refuse clipboard admission; staged — formats prepared before the clipboard is touched; gate — the lock with a deadline.

pub mod gate;
pub mod origin;
pub mod owner;
pub mod snapshot;
pub mod staged;

use std::time::{Duration, Instant};

use windows::Win32::System::DataExchange::EmptyClipboard;
use windows::Win32::System::Ole::CF_UNICODETEXT;

pub use owner::Owner;

use super::clipboard::{utf16_bytes, with_clipboard_within};
use gate::{Gate, Held};
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
        let text = crate::sanitize::strip_hidden(text);
        let owner = Owner::new()?;
        let deadline = Instant::now() + COPY_WITHIN;
        loop {
            let attempt = write_once(&owner, &text, deadline);
            if attempt.is_ok() || Instant::now() >= deadline {
                return attempt;
            }
        }
    }
}

/// One attempt to replace the clipboard with `text`, prepared before the clipboard is opened for `owner` (no longer than until `deadline`).
fn write_once(owner: &Owner, text: &str, deadline: Instant) -> Result<(), String> {
    let mut staged = Staged::new();
    staged.add_bytes(CF_UNICODETEXT.0 as u32, &utf16_bytes(text))?;
    staged.add_exclusions()?;
    let within = deadline.saturating_duration_since(Instant::now()).max(Duration::from_millis(50));
    with_clipboard_within(Some(owner.hwnd()), within, || {
        unsafe { EmptyClipboard() }.map_err(|e| e.to_string())?;
        staged.write_all()
    })
}
