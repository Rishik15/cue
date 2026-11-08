//! File: insert.rs
//! Purpose: Insert text into the focused app without taking focus (Windows), as one target-checked delivery with a structured outcome.
//! Contents: insert / replace_rich / replace — deliver text: short single-line text (up to 100 characters, espanso's limit) is typed as Unicode key events in one `SendInput` batch with the erase and the caret move, anything else is pasted through `paste` (also when the focused app's IME is in a native mode, and on a weak or remote machine where a typed burst is dropped — `fragile_env`);
//! an elevated target is detected up front and the text is left on the clipboard instead. Under one lock a delivery checks focus is still in the target window, places the text, rescues it to the clipboard on failure, then copies it when asked and presses the submit key (`conclude`).
//! insert_slowly — erase, then type one character at a time. key_chord / send / taps / key — SendInput helpers shared with `paste`; every key is marked as Cue's own and the shortcut's held modifiers are lifted around it (`input::Lift`), so nothing waits for the user to let go.
//! Call from a worker thread: it sleeps while waiting for the target app.

use std::mem::size_of;
use std::sync::Mutex;
use std::time::Duration;

use windows::Win32::UI::Input::KeyboardAndMouse::*;

use super::focus::foreground_window;
use super::input::Lift;
use super::{clipboard, elevation, ime, paste};
use crate::platform::{InsertError, InsertOptions, Submit};

/// Text longer than this is pasted, not typed (espanso's `clipboard_threshold`).
const TYPE_MAX_UNITS: usize = 100;
/// Slow insertion, per character: how long the key stays down, the same on every machine (a pace tuned to the hardware it was measured on fails on the next one).
const SLOW_HOLD: Duration = Duration::from_millis(5);
/// Slow insertion, per character: the pause after the key.
const SLOW_PAUSE: Duration = Duration::from_millis(15);
/// How long the target gets to take pasted text (and a Return after it) before more keys follow.
const SETTLE: Duration = Duration::from_millis(150);

/// One insertion at a time: two overlapping ones (back-to-back dictations) would fight over the clipboard and the keyboard queue.
static INSERTING: Mutex<()> = Mutex::new(());

/// Presses the submit key a moment after an insertion so the target has taken the text. Nothing for `Submit::None`. It takes the one-at-a-time lock itself, so it does not interleave with another insertion.
fn press_submit(submit: Submit) -> Result<(), String> {
    let with_ctrl = match submit {
        Submit::None => return Ok(()),
        Submit::Enter => false,
        Submit::CtrlEnter => true,
    };
    let _one_at_a_time = INSERTING.lock().unwrap_or_else(|e| e.into_inner());
    std::thread::sleep(SETTLE);
    let _lift = Lift::new();
    if with_ctrl {
        return send(&key_chord(VK_CONTROL, VK_RETURN));
    }
    send(&[key(VK_RETURN, 0, KEYBD_EVENT_FLAGS(0)), key(VK_RETURN, 0, KEYEVENTF_KEYUP)])
}

/// Erases `erase` characters, types `text` one character at a time with a short pause between them and moves the caret `back` to the left, for sessions that drop fast input (remote desktops, virtual machines).
/// All of it happens under one lock and one lift (held shortcut keys must not turn the typed characters into shortcuts), and the erase goes out right before the first character. Down and up are separate events with a hold between them: a slow target that gets both at once drops the character or repeats it. Single-line text only: a line break typed this way would be an Enter.
pub fn insert_slowly(erase: usize, text: &str, back: usize) -> Result<(), String> {
    let text = crate::sanitize::strip_hidden(text);
    let _one_at_a_time = INSERTING.lock().unwrap_or_else(|e| e.into_inner());
    if elevation::target_blocks_input() {
        return Err("Cue could not type here: the app runs as administrator.".into());
    }
    let _lift = Lift::new();
    let target = foreground_window();
    send(&taps(VK_BACK, erase))?;
    for unit in text.encode_utf16() {
        if foreground_window() != target {
            return Err("Focus moved to another window, so Cue stopped typing.".into());
        }
        send(&[key(VIRTUAL_KEY(0), unit, KEYEVENTF_UNICODE)])?;
        std::thread::sleep(SLOW_HOLD);
        send(&[key(VIRTUAL_KEY(0), unit, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP)])?;
        std::thread::sleep(SLOW_PAUSE);
    }
    send(&taps(VK_LEFT, back))
}

/// Delivers `text` where the cursor is, typed or pasted as fits it. See `deliver` for the guarantees.
pub fn insert(text: &str, options: &InsertOptions) -> Result<(), InsertError> {
    deliver(text, None, options, Around::default())
}

/// What goes around inserted text when it replaces something the user typed: characters to erase first and how far to move the caret left afterwards.
#[derive(Clone, Copy, Default)]
struct Around {
    erase: usize,
    back: usize,
}

/// Replaces the last `erase` typed characters with `text` and leaves the caret `back` characters before its end (for a `{cursor}`). Typed text goes in with the erasing and the caret moves as one batch of input, so nothing the user
/// types in the meantime can land between them; pasted text is erased for first, then pasted.
pub fn replace(erase: usize, text: &str, back: usize, options: &InsertOptions) -> Result<(), InsertError> {
    deliver(text, None, options, Around { erase, back })
}

/// Replaces the last `erase` typed characters with `text` pasted with `html` (a fragment) beside it, so apps that take formatting get it and the rest get the text; then moves the caret `back`.
pub fn replace_rich(erase: usize, text: &str, html: &str, back: usize, options: &InsertOptions) -> Result<(), InsertError> {
    deliver(text, Some(html), options, Around { erase, back })
}

/// The one delivery every insertion passes through, under the one-at-a-time lock. Typed or pasted text can reach a terminal or an editor, so control characters and hidden formatting never go through.
/// Focus must still be in `options.target` (when set), else nothing is typed. Whatever goes wrong before the text is in, dictated text is never lost: it waits on the clipboard, and the error says whether it does.
fn deliver(text: &str, html: Option<&str>, options: &InsertOptions, around: Around) -> Result<(), InsertError> {
    let text = crate::sanitize::strip_hidden(text);
    let html = html.map(crate::sanitize::strip_hidden);
    let typed = format!("{text}{}", options.separator);
    let placed = {
        let _one_at_a_time = INSERTING.lock().unwrap_or_else(|e| e.into_inner());
        let placed = if focus_moved(options.target) {
            Err(InsertError::FocusMoved { copied: false })
        } else {
            place(&typed, html.as_deref(), options, around)
        };
        rescued(placed, &typed)
    };
    conclude(placed, &text, options)
}

/// Whether focus is in another window than `target` (0 means any window is fine; an unknown foreground counts as the same).
fn focus_moved(target: isize) -> bool {
    let now = foreground_window();
    target != 0 && now != 0 && now != target
}

/// Puts `text` on the clipboard, retrying once: the clipboard is often only busy for a moment (a clipboard manager reading it).
fn keep_on_clipboard(text: &str) -> bool {
    clipboard::set_plain_text(text).is_ok() || {
        std::thread::sleep(Duration::from_millis(100));
        clipboard::set_plain_text(text).is_ok()
    }
}

/// For a delivery that failed before the text was in, leaves the text on the clipboard and records whether that worked.
fn rescued(placed: Result<(), InsertError>, text: &str) -> Result<(), InsertError> {
    match placed {
        Err(InsertError::Failed { why, .. }) => Err(InsertError::Failed { why, copied: keep_on_clipboard(text) }),
        Err(InsertError::FocusMoved { .. }) => Err(InsertError::FocusMoved { copied: keep_on_clipboard(text) }),
        other => other,
    }
}

/// After the text is in (also when only the clipboard restore failed): copies `text` as given, without the separator, when `keep_copy` is set, so it is what stays on the clipboard, then presses the submit key. The submit key is never pressed for text that did not go in.
/// All steps run; the first error is the one reported. Both steps run after the one-at-a-time lock is released, as in 0.3.5, so a back-to-back insertion is not held up by the copy; the submit key takes the lock itself (`press_submit`).
fn conclude(placed: Result<(), InsertError>, text: &str, options: &InsertOptions) -> Result<(), InsertError> {
    if placed.as_ref().is_err_and(|e| !e.text_went_in()) {
        return placed;
    }
    let copy = (options.keep_copy && clipboard::set_plain_text(text).is_err()).then_some(InsertError::KeepCopy);
    let submit = press_submit(options.submit).err().map(InsertError::Submit);
    placed.err().or(copy).or(submit).map_or(Ok(()), Err)
}

/// Types or pastes the text. An elevated target is detected up front and the text is left on the clipboard instead.
fn place(text: &str, html: Option<&str>, options: &InsertOptions, around: Around) -> Result<(), InsertError> {
    if elevation::target_blocks_input() {
        clipboard::set_plain_text(text)?;
        return Err(InsertError::Elevated);
    }
    let typeable = fits_typing(text) && !ime::native_input_active() && !fragile_env();
    if typeable && !options.always_paste && html.is_none() {
        let _lift = Lift::new();
        return Ok(type_unicode(text, around)?);
    }
    let _lift = Lift::new();
    paste::paste(text, html, options.restore_clipboard, around.erase)?;
    if around.back > 0 {
        std::thread::sleep(SETTLE);
        send(&taps(VK_LEFT, around.back))?;
    }
    Ok(())
}

/// A keyboard event marked as Cue's own, so the keyboard hook ignores it.
pub(super) fn key(vk: VIRTUAL_KEY, scan: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: vk, wScan: scan, dwFlags: flags, time: 0, dwExtraInfo: super::input::MARK } },
    }
}

/// Press and release `k` alone, `count` times. Arrow and navigation keys are sent as extended keys: without that flag Windows reads an arrow as its numpad twin, so a `{cursor}` caret move (Left) types a digit when NumLock is on.
pub(super) fn taps(k: VIRTUAL_KEY, count: usize) -> Vec<INPUT> {
    let extended = matches!(k, VK_LEFT | VK_RIGHT | VK_UP | VK_DOWN | VK_HOME | VK_END | VK_PRIOR | VK_NEXT | VK_INSERT | VK_DELETE);
    let down = if extended { KEYEVENTF_EXTENDEDKEY } else { KEYBD_EVENT_FLAGS(0) };
    (0..count).flat_map(|_| [key(k, 0, down), key(k, 0, down | KEYEVENTF_KEYUP)]).collect()
}

/// Press and release `key` while `modifier` is held.
pub fn key_chord(modifier: VIRTUAL_KEY, k: VIRTUAL_KEY) -> [INPUT; 4] {
    let up = KEYEVENTF_KEYUP;
    [key(modifier, 0, KEYBD_EVENT_FLAGS(0)), key(k, 0, KEYBD_EVENT_FLAGS(0)), key(k, 0, up), key(modifier, 0, up)]
}

/// Sends the inputs in one `SendInput` call; Err when Windows injected fewer than given. Safety: `inputs` is a valid slice of fully initialised `INPUT` values (each keyboard union member written by `key`), and the size passed is that of one `INPUT`.
pub fn send(inputs: &[INPUT]) -> Result<(), String> {
    if inputs.is_empty() {
        return Ok(());
    }
    let sent = unsafe { SendInput(inputs, size_of::<INPUT>() as i32) };
    if sent as usize == inputs.len() {
        Ok(())
    } else {
        Err(format!("SendInput injected {sent}/{}", inputs.len()))
    }
}

/// Erases, types and moves the caret as one `SendInput` call, which Windows keeps together, so the user's own keys cannot come between (espanso injects the same way).
fn type_unicode(text: &str, around: Around) -> Result<(), String> {
    let mut inputs = taps(VK_BACK, around.erase);
    inputs.extend(
        text.encode_utf16()
            .flat_map(|u| [key(VIRTUAL_KEY(0), u, KEYEVENTF_UNICODE), key(VIRTUAL_KEY(0), u, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP)]),
    );
    inputs.extend(taps(VK_LEFT, around.back));
    send(&inputs)
}

/// Short single-line text is typed; anything else is pasted.
fn fits_typing(text: &str) -> bool {
    text.encode_utf16().count() <= TYPE_MAX_UNITS && !text.contains(['\n', '\r'])
}

/// A machine where a typed burst of key events is dropped: a small PC (under 4 cores, so a virtual machine with two) or a Remote Desktop session. There Auto pastes instead, one atomic clipboard read the target takes whole.
fn fragile_env() -> bool {
    super::hardware::weak_machine() || super::hardware::remote_session()
}
