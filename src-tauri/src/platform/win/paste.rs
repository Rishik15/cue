//! File: paste.rs
//! Purpose: Clipboard paste in espanso's flow (write the text, Ctrl+V, restore after the app has read it) with delayed rendering on top: Cue owns a hidden window and supplies the text only when the target asks for it, so the original clipboard is restored right after the app has actually read it, and no wait comes before Ctrl+V (that gap let typed keys land between an erase and the paste).
//! A render request counts as the target's only when the program that has the clipboard open is the target's (same top-level window or process); a history tool, a VM clipboard agent or a reader that cannot be named, reading first at any speed, is not a paste. After an agent has taken the text the target may never ask, so the paste counts as done and the old clipboard is not put back (ADR 0014).
//! The old clipboard is read and replaced in one open of the clipboard, so nothing can be copied between the two; a clipboard that cannot be kept whole is not restored and the receipt says why.
//! Contents: paste — borrow, publish, validate the target, send erase + Ctrl+V as one batch, wait for the read, restore; publish — snapshot and replace in one open; await_read — wait for the target's render receipt and the quiet period; restore — conditional put-back.

use std::time::{Duration, Instant};

use windows::Win32::System::DataExchange::EmptyClipboard;
use windows::Win32::System::Ole::CF_UNICODETEXT;
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_BACK, VK_CONTROL, VK_V};
use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

use super::borrow::{Borrow, Ours};
use super::cf_html;
use super::clipboard::{html_format, open_within, sequence, utf16_bytes};
use super::clipboard_tx::owner::{identity, Materialized};
use super::clipboard_tx::snapshot::{Refused, Snapshot};
use super::clipboard_tx::staged::Staged;
use super::clipboard_tx::{Owner, Tx};
use super::insert::{key_chord, taps, try_send};
use crate::platform::{ClipboardEnd, Failure, Placement, Receipt};

/// How long the target gets to ask for the pasted text.
const READ_TIMEOUT: Duration = Duration::from_millis(1500);
/// After the target has asked, how long nothing may ask again before the old clipboard goes back: a reader that takes the text and then the HTML (or asks twice) must not get the old clipboard in between.
const QUIET: Duration = Duration::from_millis(100);
/// The longest the old clipboard waits for a reader that keeps asking.
const QUIET_CAP: Duration = Duration::from_millis(1000);
/// After an agent has rendered the text, how long the target still gets to ask before its silence is read as "it took the stored copy".
const CACHED_SETTLE: Duration = Duration::from_millis(300);
/// How long publishing waits for another program to let go of the clipboard.
const PUBLISH_WITHIN: Duration = Duration::from_millis(1500);

/// Pastes `text` over the `erase` characters before the caret; with `html` (a fragment) the target may take the formatted version instead. The caller holds the transaction and the `Lift`; `before_send` is asked immediately before the keys go out and may refuse.
/// The old clipboard is borrowed first (when `restore` is set); one that cannot be kept whole is not restored afterwards, because the dictated text still has to reach the app. The erase of a typed trigger and the Ctrl+V are one batch of input, so keys the user types meanwhile cannot land between them. When nothing was sent or nobody asked, the text is written out of the delayed offer onto the clipboard before the owner window goes, so it is never lost with it.
pub fn paste(
    _tx: &Tx,
    text: &str,
    html: Option<&str>,
    restore: bool,
    erase: usize,
    before_send: &dyn Fn() -> Result<(), Failure>,
) -> Receipt {
    let mut receipt = Receipt::nothing();
    let owner = match Owner::new() {
        Ok(owner) => owner,
        Err(e) => return failed(receipt, Failure::Other(e)),
    };
    let payload = utf16_bytes(text);
    owner.offer(payload.clone(), html.map(cf_html::build).unwrap_or_default(), identity(unsafe { GetForegroundWindow() }));
    let published = match publish(&owner, restore) {
        Ok(published) => published,
        Err(e) => {
            receipt.clipboard_seq = Some(sequence());
            return failed(receipt, Failure::Other(e));
        }
    };
    let Published { borrow, end, seq } = published;
    receipt.clipboard = end;
    receipt.clipboard_seq = Some(seq);
    if let Err(failure) = before_send() {
        keep_text(&owner, &mut receipt);
        return failed(receipt, failure);
    }
    if !owner.still_owns() {
        receipt.newer_copy = true;
        return failed(receipt, Failure::Other("the clipboard changed just before the paste".into()));
    }
    let mut keys = taps(VK_BACK, erase);
    keys.extend(key_chord(VK_CONTROL, VK_V));
    if let Err(short) = try_send(&keys) {
        keep_text(&owner, &mut receipt);
        receipt.placement = if short.sent > 0 { Placement::Uncertain } else { Placement::NotDone };
        receipt.failure = Some(Failure::Other(short.to_string()));
        return receipt;
    }
    await_read(&owner);
    receipt.target_read = owner.rendered();
    if !(owner.rendered() || owner.cached()) {
        keep_text(&owner, &mut receipt);
        receipt.placement = Placement::Uncertain;
        if borrow.is_some() {
            receipt.clipboard = ClipboardEnd::Kept;
        }
        receipt.failure = Some(Failure::NotRead);
        return receipt;
    }
    receipt.placement = Placement::Sent;
    restore_clipboard(&mut receipt, borrow, &owner, &payload);
    receipt
}

/// The receipt of a delivery that sent nothing, with its reason.
fn failed(mut receipt: Receipt, failure: Failure) -> Receipt {
    receipt.failure = Some(failure);
    receipt
}

/// What publishing the offer found and left.
struct Published {
    /// The old clipboard, when it was admitted whole (and `restore` was asked for).
    borrow: Option<Borrow>,
    /// What to tell about the clipboard so far.
    end: ClipboardEnd,
    /// The sequence number right after Cue's own write.
    seq: u32,
}

/// Reads the clipboard (when `restore`) and replaces it with the delayed offer in one open for `owner`, so nothing copied meanwhile can be lost without being in the snapshot. When the write fails after the clipboard was emptied, the admitted snapshot is put back before the clipboard is closed (R9), so a failed publication costs the user nothing. Err when the clipboard stayed busy or could not be written.
fn publish(owner: &Owner, restore: bool) -> Result<Published, String> {
    let mut staged = Staged::new();
    staged.add_delayed(CF_UNICODETEXT.0 as u32);
    if owner.offers_html() {
        staged.add_delayed(html_format());
    }
    staged.add_exclusions()?;
    open_within(Some(owner.hwnd()), PUBLISH_WITHIN)?;
    let admitted = restore.then(|| Snapshot::read_open().map(Borrow::of));
    let written = unsafe { EmptyClipboard() }.map_err(|e| e.to_string()).and_then(|()| staged.write_all());
    if written.is_err() {
        if let Some(Ok(old)) = &admitted {
            let _ = old.put_back_in_open();
        }
    }
    let seq = sequence();
    let _ = unsafe { windows::Win32::System::DataExchange::CloseClipboard() };
    written.map_err(|e| format!("Could not publish text to the clipboard: {e}"))?;
    Ok(match admitted {
        None => Published { borrow: None, end: ClipboardEnd::Untouched, seq },
        Some(Ok(borrow)) => Published { borrow: Some(borrow), end: ClipboardEnd::Untouched, seq },
        Some(Err(refused)) => Published { borrow: None, end: ClipboardEnd::LeftBehind(left_reason(&refused)), seq },
    })
}

/// The short reason shown when the old clipboard was not kept.
fn left_reason(refused: &Refused) -> String {
    match refused {
        Refused::TooBig => "it was too large to keep".into(),
        Refused::Busy => "another program was using it".into(),
        Refused::Unreadable(_) => "it held data that could not be read".into(),
        Refused::Unsupported(_) => "it held data that cannot be copied faithfully".into(),
    }
}

/// Waits for the target to ask for the text (an agent's read alone starts a short settle for the target), then until nothing has asked for `QUIET`, at most `QUIET_CAP`.
fn await_read(owner: &Owner) {
    owner.pump_until(Instant::now() + READ_TIMEOUT, || owner.rendered() || owner.cached());
    if !owner.rendered() && owner.cached() {
        owner.pump_until(Instant::now() + CACHED_SETTLE, || owner.rendered());
    }
    let cap = Instant::now() + QUIET_CAP;
    while Instant::now() < cap && owner.read_within(QUIET) {
        owner.pump_until(Instant::now() + Duration::from_millis(10), || false);
    }
}

/// Puts the old clipboard back when the target itself read the text, and records what became of it. An agent's read alone leaves the clipboard (ADR 0014); a newer copy is kept.
fn restore_clipboard(receipt: &mut Receipt, borrow: Option<Borrow>, owner: &Owner, payload: &[u8]) {
    let Some(borrow) = borrow else { return };
    if !owner.rendered() {
        receipt.clipboard = ClipboardEnd::Kept;
        return;
    }
    receipt.clipboard = match borrow.finish(Ours::Owner(owner.hwnd()), owner, Some(payload)) {
        Ok(super::borrow::Restored::Back) => ClipboardEnd::Restored,
        Ok(super::borrow::Restored::NewerCopy) => ClipboardEnd::Kept,
        Err(why) => ClipboardEnd::LeftBehind(why),
    };
}

/// Leaves the text on the clipboard out of the delayed offer, before the owner window goes, and records what happened: the text is on the clipboard, or somebody else owns the clipboard now and a newer copy of theirs is not overwritten (the caller keeps the text), or the clipboard stayed busy (a rescue copy may try again).
fn keep_text(owner: &Owner, receipt: &mut Receipt) {
    match owner.materialize() {
        Ok(Materialized::Kept) => receipt.copied = true,
        Ok(Materialized::Taken) => receipt.newer_copy = true,
        Err(_) => {}
    }
}
