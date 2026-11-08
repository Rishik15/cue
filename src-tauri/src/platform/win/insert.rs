//! File: insert.rs
//! Purpose: Insert text into the focused app without taking focus (Windows), as one target-checked delivery under the clipboard coordinator that returns an honest receipt.
//! Contents: insert / replace_rich / replace / insert_slowly — deliver text: short single-line text (up to 100 UTF-16 units, espanso's limit) is typed as Unicode key events in one `SendInput` batch with the erase and the caret move, anything else is pasted through `paste` (also when the focused app's IME is open in a native mode or does not answer, and on a weak or remote machine where a typed burst is dropped — `fragile_env`), and slow typing is a third way to put the same text in.
//! deliver — the one path: prepare, validate the target immediately before the keys go out (an unknown foreground fails closed), place, rescue to the clipboard on failure, then keep a copy and press the submit key only for text that was sent (`conclude`). An elevated target is detected up front and the text is left on the clipboard instead.
//! key_chord / try_send / send / taps / key — SendInput helpers (in `insert/keys.rs`) shared with `paste`; every key is marked as Cue's own and the shortcut's held modifiers are lifted around it (`input::Lift`), so nothing waits for the user to let go. Call from a worker thread: it sleeps while waiting for the target app.
//! The caret move after a paste or slow typing is skipped when the target moved or the user typed a key since the text went out (`input::activity_mark`), and the receipt says so.
//! A short `SendInput` batch is balanced (`unreleased`): the keys it pressed get their key-ups. shortcut: the re-press of held modifiers that `input::Lift` sends goes through the same path, so a short re-press releases a modifier the user still holds, a rare double fault.

use std::time::Duration;

use windows::Win32::UI::Input::KeyboardAndMouse::*;

mod caret;
mod keys;

pub(in crate::platform::win) use keys::key;
pub use keys::{key_chord, send, taps, try_send, Short};

use super::clipboard::sequence;
use super::clipboard_tx::{self, Tx};
use super::focus::target_now;
use super::ime::{self, ImeState};
use super::input::Lift;
use super::{elevation, paste};
use crate::input::activity_mark;
use crate::platform::{Caret, Failure, InsertError, InsertOptions, Placement, Receipt, Submit, Target};

/// Text longer than this is pasted, not typed (espanso's `clipboard_threshold`).
const TYPE_MAX_UNITS: usize = 100;
/// Slow insertion, per character: how long the key stays down, the same on every machine.
const SLOW_HOLD: Duration = Duration::from_millis(5);
/// Slow insertion, per character: the pause after the key.
const SLOW_PAUSE: Duration = Duration::from_millis(15);
/// How long the target gets to take pasted text (and a Return after it) before more keys follow.
const SETTLE: Duration = Duration::from_millis(150);

/// What goes around inserted text when it replaces something the user typed: characters to erase first and how far to move the caret left afterwards.
#[derive(Clone, Copy, Default)]
struct Around {
    erase: usize,
    back: usize,
}

/// Delivers `text` where the cursor is, typed or pasted as fits it. See `deliver` for the guarantees.
pub fn insert(text: &str, options: &InsertOptions) -> Result<(), InsertError> {
    deliver(text, None, options, Around::default()).into_result()
}

/// Replaces the last `erase` typed characters with `text` and leaves the caret `back` characters before its end (for a `{cursor}`). Typed text goes in with the erasing and the caret moves as one batch of input, so nothing the user types in the meantime can land between them; pasted text is erased for first, then pasted.
pub fn replace(erase: usize, text: &str, back: usize, options: &InsertOptions) -> Result<(), InsertError> {
    deliver(text, None, options, Around { erase, back }).into_result()
}

/// Replaces the last `erase` typed characters with `text` pasted with `html` (a fragment) beside it, so apps that take formatting get it and the rest get the text; then moves the caret `back`.
pub fn replace_rich(erase: usize, text: &str, html: &str, back: usize, options: &InsertOptions) -> Result<(), InsertError> {
    deliver(text, Some(html), options, Around { erase, back }).into_result()
}

/// Erases, then types one character at a time with a pause, for sessions that drop fast input; the same delivery as the others (target, rescue to the clipboard, receipt). The window and field in front when it is called are the target of every character and of the caret move. Single-line text only.
pub fn insert_slowly(erase: usize, text: &str, back: usize) -> Result<(), String> {
    let token = super::focus::capture_target();
    if token.window == 0 {
        return Err("there is no window to type into".into());
    }
    let options = InsertOptions { slow: true, restore_clipboard: false, target: token.window, token: Some(token), ..Default::default() };
    deliver(text, None, &options, Around { erase, back }).into_result().map_err(|e| e.to_string())
}

/// The one delivery every insertion passes through, under the coordinator. Typed or pasted text can reach a terminal or an editor, so control characters and hidden formatting never go through. The target is checked before the keys go out and refused when unknown; whatever goes wrong before the text is in, it waits on the clipboard, and the receipt says whether it does.
fn deliver(text: &str, html: Option<&str>, options: &InsertOptions, around: Around) -> Receipt {
    let text = crate::sanitize::strip_hidden(text);
    let html = html.map(crate::sanitize::strip_hidden);
    let tx = match clipboard_tx::enter() {
        Ok(tx) => tx,
        Err(_) => return refused(Failure::Busy),
    };
    let start = sequence();
    if let Err(failure) = still_there(&options.place()) {
        let mut receipt = refused(failure);
        rescue(&tx, &mut receipt, &format!("{text}{}", options.separator), start);
        return receipt;
    }
    let text = match options.fit.and_then(|fit| super::context::text_before_cursor().map(|before| fit(&text, &before))) {
        Some(fitted) => fitted,
        None => text,
    };
    let typed = format!("{text}{}", options.separator);
    let mut receipt = place(&tx, &typed, html.as_deref(), options, around);
    rescue(&tx, &mut receipt, &typed, start);
    conclude(&tx, &mut receipt, &text, options);
    receipt
}

/// The receipt of a delivery that did nothing, with its reason.
fn refused(failure: Failure) -> Receipt {
    Receipt { failure: Some(failure), ..Receipt::nothing() }
}

/// Whether the keys would still go where the text is for: the same window, process and thread, and the same focused element when the token has one. An unknown foreground never passes.
fn still_there(want: &Target) -> Result<(), Failure> {
    let now = target_now(want.runtime_id.is_some());
    if now.window != 0 && want.same_as(&now) {
        Ok(())
    } else {
        Err(Failure::FocusMoved)
    }
}

/// For a delivery whose text may not be in, leaves the text on the clipboard (once, unless the paste already did) and records what happened. It writes only while the clipboard is still as Cue last left it (`start` when Cue wrote nothing), checked while the clipboard is open: a copy the user made meanwhile stays.
fn rescue(tx: &Tx, receipt: &mut Receipt, text: &str, start: u32) {
    if receipt.placement == Placement::Sent || receipt.copied || receipt.newer_copy {
        return;
    }
    let seen = receipt.clipboard_seq.unwrap_or(start);
    record_rescue(receipt, tx.copy_text_unless_newer(text, Some(seen)));
}

/// Records the outcome of a rescue copy: written, left alone because something newer is on the clipboard, or failed.
fn record_rescue(receipt: &mut Receipt, outcome: Result<bool, String>) {
    match outcome {
        Ok(true) => receipt.copied = true,
        Ok(false) => receipt.newer_copy = true,
        Err(_) => {}
    }
}

/// After the text is sent: copies `text` as given, without the separator, when `keep_copy` is set, so it is what stays on the clipboard, then presses the submit key. Neither runs for text that was not sent. All steps run; each failure is recorded.
fn conclude(tx: &Tx, receipt: &mut Receipt, text: &str, options: &InsertOptions) {
    if receipt.placement != Placement::Sent {
        return;
    }
    if options.keep_copy {
        receipt.keep_copy_failed = tx.copy_text(text).is_err();
        receipt.copied |= !receipt.keep_copy_failed;
    }
    receipt.submit = press_submit(options.submit, &options.place()).err();
}

/// Presses the submit key a moment after an insertion so the target has taken the text. Nothing for `Submit::None`, and nothing when the keys would no longer go to the target. The caller holds the coordinator.
fn press_submit(submit: Submit, want: &Target) -> Result<(), String> {
    let with_ctrl = match submit {
        Submit::None => return Ok(()),
        Submit::Enter => false,
        Submit::CtrlEnter => true,
    };
    std::thread::sleep(SETTLE);
    let _lift = Lift::new();
    still_there(want).map_err(|_| "focus moved to another window before the key".to_string())?;
    if with_ctrl {
        return send(&key_chord(VK_CONTROL, VK_RETURN));
    }
    send(&[key(VK_RETURN, 0, KEYBD_EVENT_FLAGS(0)), key(VK_RETURN, 0, KEYEVENTF_KEYUP)])
}

/// Types or pastes the text, choosing the way that fits it and the machine; everything slow (the IME query) is done before the target is checked for the last time.
fn place(tx: &Tx, text: &str, html: Option<&str>, options: &InsertOptions, around: Around) -> Receipt {
    let want = options.place();
    if elevation::target_blocks_input() {
        return refused(Failure::Elevated);
    }
    if options.slow {
        return type_slowly(text, around, &want);
    }
    let may_type = !options.always_paste && html.is_none() && fits_typing(text);
    if may_type && !fragile_env() && ime::state() == ImeState::Inactive {
        return type_unicode(text, around, &want);
    }
    let _lift = Lift::new();
    let mark = activity_mark();
    let mut receipt = paste::paste(tx, text, html, options.restore_clipboard, around.erase, &|| still_there(&want));
    if receipt.placement == Placement::Sent && around.back > 0 {
        receipt.caret = caret::move_caret(&want, mark, around.back);
    }
    receipt
}

/// Erases, types and moves the caret as one `SendInput` call, which Windows keeps together, so the user's own keys cannot come between (espanso injects the same way). The target is checked after the held keys are lifted, right before the call.
fn type_unicode(text: &str, around: Around, want: &Target) -> Receipt {
    let _lift = Lift::new();
    if let Err(failure) = still_there(want) {
        return refused(failure);
    }
    let mut inputs = taps(VK_BACK, around.erase);
    inputs.extend(
        text.encode_utf16()
            .flat_map(|u| [key(VIRTUAL_KEY(0), u, KEYEVENTF_UNICODE), key(VIRTUAL_KEY(0), u, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP)]),
    );
    inputs.extend(taps(VK_LEFT, around.back));
    sent_or_short(try_send(&inputs), around.back)
}

/// The receipt of one batch: sent whole, or short with the count.
fn sent_or_short(result: Result<(), Short>, back: usize) -> Receipt {
    match result {
        Ok(()) => {
            Receipt { placement: Placement::Sent, caret: if back > 0 { Caret::Moved } else { Caret::NotNeeded }, ..Receipt::nothing() }
        }
        Err(short) => Receipt {
            placement: if short.sent > 0 { Placement::Uncertain } else { Placement::NotDone },
            failure: Some(Failure::Other(short.to_string())),
            ..Receipt::nothing()
        },
    }
}

/// Erases, types one character at a time with a short pause and moves the caret `back`, all under one lift. The erase goes out right before the first character. Down and up are separate events with a hold between them: a slow target that gets both at once drops the character or repeats it. Focus is checked before every character; a line break typed this way would be an Enter, so it is refused first.
fn type_slowly(text: &str, around: Around, want: &Target) -> Receipt {
    if text.contains(['\n', '\r']) {
        return refused(Failure::Other("slow typing is for single-line text".into()));
    }
    let _lift = Lift::new();
    if let Err(failure) = still_there(want) {
        return refused(failure);
    }
    let mark = activity_mark();
    if let Err(short) = try_send(&taps(VK_BACK, around.erase)) {
        return sent_or_short(Err(short), 0);
    }
    for unit in text.encode_utf16() {
        let stepped = still_there_cheap(want).and_then(|()| type_unit(unit));
        if let Err(why) = stepped {
            return Receipt { placement: Placement::Uncertain, failure: Some(Failure::Other(why)), ..Receipt::nothing() };
        }
    }
    let caret = if around.back > 0 { caret::move_caret_now(want, mark, around.back) } else { Caret::NotNeeded };
    Receipt { placement: Placement::Sent, caret, ..Receipt::nothing() }
}

/// Whether the foreground window is still the target's, without asking UI Automation (it runs per character).
fn still_there_cheap(want: &Target) -> Result<(), String> {
    if want.window != 0 && super::focus::foreground_window() == want.window {
        Ok(())
    } else {
        Err("focus moved to another window, so Cue stopped typing".into())
    }
}

/// Types one UTF-16 unit with the hold and pause of slow insertion.
fn type_unit(unit: u16) -> Result<(), String> {
    send(&[key(VIRTUAL_KEY(0), unit, KEYEVENTF_UNICODE)])?;
    std::thread::sleep(SLOW_HOLD);
    let up = [key(VIRTUAL_KEY(0), unit, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP)];
    send(&up).or_else(|_| send(&up))?;
    std::thread::sleep(SLOW_PAUSE);
    Ok(())
}

/// Short single-line text is typed; anything else is pasted.
fn fits_typing(text: &str) -> bool {
    text.encode_utf16().take(TYPE_MAX_UNITS + 1).count() <= TYPE_MAX_UNITS && !text.contains(['\n', '\r'])
}

/// A machine where a typed burst of key events is dropped: a small PC (under 4 cores, so a virtual machine with two) or a Remote Desktop session. There Auto pastes instead, one atomic clipboard read the target takes whole.
fn fragile_env() -> bool {
    super::hardware::weak_machine() || super::hardware::remote_session()
}

#[cfg(test)]
mod tests {
    use super::{fits_typing, record_rescue, sent_or_short, Short};
    use crate::platform::{Caret, Failure, Placement};

    /// 100 UTF-16 units are typed, 101 are pasted, and a surrogate pair counts as two.
    #[test]
    fn typing_stops_at_100_utf16_units_and_pairs_count_twice() {
        assert!(fits_typing(&"a".repeat(100)));
        assert!(!fits_typing(&"a".repeat(101)));
        assert!(fits_typing(&"😀".repeat(50)));
        assert!(!fits_typing(&format!("{}a", "😀".repeat(50))));
        assert!(!fits_typing("two\nlines"));
    }

    /// A whole batch is sent with its caret move; a short one keeps its counts and is uncertain when some events went out.
    #[test]
    fn a_short_batch_keeps_its_counts_and_is_uncertain() {
        let whole = sent_or_short(Ok(()), 2);
        assert_eq!((whole.placement, whole.caret), (Placement::Sent, Caret::Moved));
        let partial = sent_or_short(Err(Short { sent: 3, total: 12 }), 0);
        assert_eq!(partial.placement, Placement::Uncertain);
        assert_eq!(partial.failure, Some(Failure::Other("SendInput injected 3/12".into())));
        assert_eq!(sent_or_short(Err(Short { sent: 0, total: 12 }), 0).placement, Placement::NotDone);
    }

    /// A rescue copy records its three outcomes: written, left alone for a newer copy, failed.
    #[test]
    fn a_rescue_copy_records_what_became_of_it() {
        use crate::platform::Receipt;
        let mut written = Receipt::nothing();
        record_rescue(&mut written, Ok(true));
        assert!(written.copied && !written.newer_copy);
        let mut newer = Receipt::nothing();
        record_rescue(&mut newer, Ok(false));
        assert!(!newer.copied && newer.newer_copy);
        let mut failed = Receipt::nothing();
        record_rescue(&mut failed, Err("busy".into()));
        assert!(!failed.copied && !failed.newer_copy);
    }
}
