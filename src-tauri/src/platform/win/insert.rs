//! Purpose: Insert text into the focused app without taking focus (Windows).
//! Contents: press — a key several times; insert_slowly — type one character at a time; insert / insert_rich / replace — short single-line text (up to 120 characters) is typed as Unicode key events in pieces of 24 with a pause between them, so a slow target cannot drop the rest (replace also erases what the user typed first and moves the caret; text of one piece goes in one batch), anything else is
//! pasted through `paste` (also when the focused app's IME is in a native mode, where typed events would enter its composition); an elevated target is detected up front and the text is left on the clipboard instead.
//! key_chord / send — SendInput helpers shared with `paste`; every key is marked as Cue's own and the shortcut's held modifiers are lifted around it (`input::Lift`), so nothing waits for the user to let go. `press` and `press_submit` take the same one-insertion-at-a-time lock as the inserts, and an insert holds one `Lift` around erase, paste and caret move. Call from a worker thread: it sleeps while waiting for the target app.

use std::fmt;
use std::mem::size_of;
use std::sync::Mutex;
use std::time::Duration;

use windows::Win32::UI::Input::KeyboardAndMouse::*;

use super::focus::foreground_window;
use super::input::Lift;
use super::{clipboard, elevation, ime, paste};
use crate::platform::{InsertOptions, Key};

/// Text longer than this is pasted, not typed: a paste is one step the target cannot drop half of.
const TYPE_MAX_UNITS: usize = 120;
/// Typed text goes out in pieces of this many UTF-16 units with a pause between them: a target that is slow (a small or virtual machine) drops the rest of a long batch, and only the first words arrive.
const CHUNK_UNITS: usize = 24;
const CHUNK_PAUSE: Duration = Duration::from_millis(4);
/// Slow insertion, per character: how long the key stays down and the pause after it, the same on every machine (a pace tuned to the hardware it was measured on fails on the next one). `SLOW_START` is the wait before the first character: the erased trigger is still being processed by a slow target, and a late Backspace would eat typed text.
const SLOW_HOLD: Duration = Duration::from_millis(5);
const SLOW_PAUSE: Duration = Duration::from_millis(15);
const SLOW_START: Duration = Duration::from_millis(100);
/// How long the target gets to take pasted text (and a Return after it) before more keys follow.
const SETTLE: Duration = Duration::from_millis(150);

/// One insertion at a time: two overlapping ones (back-to-back dictations) would fight over the clipboard and the keyboard queue.
static INSERTING: Mutex<()> = Mutex::new(());

#[derive(Debug)]
pub enum InsertError {
    /// The focused app runs as administrator; Windows blocks our input. The text was copied for a manual paste.
    Elevated,
    /// Nobody asked for the pasted text, so it was left on the clipboard.
    NotTaken,
    /// The text went in, but the clipboard the user had before could not be put back.
    NotRestored,
    Failed(String),
}

impl InsertError {
    /// The text reached the app even though this error is reported (only the clipboard restore failed).
    pub fn text_went_in(&self) -> bool {
        matches!(self, InsertError::NotRestored)
    }

    /// The short line for the pill on screen: it says where the text went.
    pub fn pill(&self) -> &'static str {
        match self {
            InsertError::Elevated | InsertError::NotTaken => "Could not insert, copied to clipboard",
            InsertError::NotRestored => "Inserted, but your old clipboard was not restored",
            InsertError::Failed(e) if e.ends_with("so the text is on your clipboard") => "Could not insert, copied to clipboard",
            InsertError::Failed(_) => "Could not insert the text",
        }
    }
}

impl fmt::Display for InsertError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            InsertError::Elevated => {
                f.write_str("This app runs as administrator, so Cue cannot type into it. The text is on your clipboard.")
            }
            InsertError::NotTaken => f.write_str("The app did not take the pasted text, so it was left on your clipboard."),
            InsertError::NotRestored => f.write_str("The text was inserted, but Cue could not put your previous clipboard back."),
            InsertError::Failed(e) => write!(f, "Could not insert text: {e}"),
        }
    }
}

impl From<String> for InsertError {
    fn from(e: String) -> Self {
        InsertError::Failed(e)
    }
}

/// Presses Enter (or Ctrl+Enter) in the focused window, a moment after an insertion so the target has taken the text.
pub fn press_submit(with_ctrl: bool) -> Result<(), String> {
    let _one_at_a_time = INSERTING.lock().unwrap_or_else(|e| e.into_inner());
    std::thread::sleep(SETTLE);
    let _lift = Lift::new();
    if with_ctrl {
        return send(&key_chord(VK_CONTROL, VK_RETURN));
    }
    send(&[key(VK_RETURN, 0, KEYBD_EVENT_FLAGS(0)), key(VK_RETURN, 0, KEYEVENTF_KEYUP)])
}

/// Presses `key` `count` times in the focused window (erasing a typed trigger, moving the caret back to a `{cursor}`).
pub fn press(key_to_press: Key, count: usize) -> Result<(), String> {
    let vk = match key_to_press {
        Key::Backspace => VK_BACK,
        Key::Left => VK_LEFT,
    };
    let _one_at_a_time = INSERTING.lock().unwrap_or_else(|e| e.into_inner());
    let _lift = Lift::new();
    send(&taps(vk, count))
}

/// Types `text` one character at a time with a short pause between them, for sessions that drop fast input (remote desktops, virtual machines). Single-line text only: a line break typed this way would be an Enter.
pub fn insert_slowly(text: &str) -> Result<(), String> {
    let text = crate::sanitize::strip_hidden(text);
    let _one_at_a_time = INSERTING.lock().unwrap_or_else(|e| e.into_inner());
    if elevation::target_blocks_input() {
        return Err("Cue could not type here: the app runs as administrator.".into());
    }
    let _lift = Lift::new(); // held shortcut keys must not turn the typed characters into shortcuts
    let target = foreground_window();
    std::thread::sleep(SLOW_START);
    for unit in text.encode_utf16() {
        if foreground_window() != target {
            return Err("Focus moved to another window, so Cue stopped typing.".into());
        }
        // Down and up are separate events with a hold between them: a slow target that gets both at once drops the character or repeats it.
        send(&[key(VIRTUAL_KEY(0), unit, KEYEVENTF_UNICODE)])?;
        std::thread::sleep(SLOW_HOLD);
        send(&[key(VIRTUAL_KEY(0), unit, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP)])?;
        std::thread::sleep(SLOW_PAUSE);
    }
    Ok(())
}

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

/// Pastes `text` with `html` (a fragment) beside it, so apps that take formatting get it and the rest get the text.
pub fn insert_rich(text: &str, html: &str, options: &InsertOptions) -> Result<(), InsertError> {
    deliver(text, Some(html), options, Around::default())
}

fn deliver(text: &str, html: Option<&str>, options: &InsertOptions, around: Around) -> Result<(), InsertError> {
    // Typed or pasted text can reach a terminal or an editor, so control characters and hidden formatting never go through (the one place every insertion passes).
    let text = crate::sanitize::strip_hidden(text);
    let _one_at_a_time = INSERTING.lock().unwrap_or_else(|e| e.into_inner());
    let html = html.map(crate::sanitize::strip_hidden);
    match place(&text, html.as_deref(), options, around) {
        // Whatever went wrong, dictated text is never lost: it waits on the clipboard.
        Err(InsertError::Failed(why)) => {
            // One retry: the clipboard is often only busy for a moment (a clipboard manager reading it).
            let kept = clipboard::set_plain_text(&text).is_ok() || {
                std::thread::sleep(Duration::from_millis(100));
                clipboard::set_plain_text(&text).is_ok()
            };
            let saved = if kept { "so the text is on your clipboard" } else { "and the text could not be saved to the clipboard" };
            Err(InsertError::Failed(format!("{why}, {saved}")))
        }
        other => other,
    }
}

fn place(text: &str, html: Option<&str>, options: &InsertOptions, around: Around) -> Result<(), InsertError> {
    if elevation::target_blocks_input() {
        clipboard::set_plain_text(text)?;
        return Err(InsertError::Elevated);
    }
    let typeable = text.encode_utf16().count() <= TYPE_MAX_UNITS && !text.contains(['\n', '\r']) && !ime::native_input_active();
    if typeable && !options.always_paste && html.is_none() {
        let _lift = Lift::new();
        return Ok(type_unicode(text, around)?);
    }
    let _lift = Lift::new(); // one lift around erase, paste and caret move
    send(&taps(VK_BACK, around.erase))?;
    paste::paste(text, html, options.restore_clipboard)?;
    if around.back > 0 {
        std::thread::sleep(SETTLE); // the target may still be processing the paste
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

/// Press and release `k` alone, `count` times.
fn taps(k: VIRTUAL_KEY, count: usize) -> Vec<INPUT> {
    (0..count).flat_map(|_| [key(k, 0, KEYBD_EVENT_FLAGS(0)), key(k, 0, KEYEVENTF_KEYUP)]).collect()
}

/// Press and release `key` while `modifier` is held.
pub fn key_chord(modifier: VIRTUAL_KEY, k: VIRTUAL_KEY) -> [INPUT; 4] {
    let up = KEYEVENTF_KEYUP;
    [key(modifier, 0, KEYBD_EVENT_FLAGS(0)), key(k, 0, KEYBD_EVENT_FLAGS(0)), key(k, 0, up), key(modifier, 0, up)]
}

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

/// Erases, types and moves the caret. Short text (one piece) goes out in one `SendInput` call, which Windows keeps together, so the user's own keys cannot come between;
/// longer text goes in pieces with a pause between them (the erase with the first piece, the caret move with the last), and stops if focus moves to another window.
fn type_unicode(text: &str, around: Around) -> Result<(), String> {
    let units: Vec<u16> = text.encode_utf16().collect();
    let pieces = split_units(&units, CHUNK_UNITS);
    let (target, last) = (foreground_window(), pieces.len() - 1);
    for (i, piece) in pieces.iter().enumerate() {
        if i > 0 {
            std::thread::sleep(CHUNK_PAUSE);
            if foreground_window() != target {
                return Err("Focus moved to another window, so Cue stopped typing.".into());
            }
        }
        let mut inputs = if i == 0 { taps(VK_BACK, around.erase) } else { Vec::new() };
        inputs.extend(
            piece
                .iter()
                .flat_map(|&u| [key(VIRTUAL_KEY(0), u, KEYEVENTF_UNICODE), key(VIRTUAL_KEY(0), u, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP)]),
        );
        if i == last {
            inputs.extend(taps(VK_LEFT, around.back));
        }
        send(&inputs)?;
    }
    Ok(())
}

/// `units` in pieces of at most `max` (at least 2), never cutting a surrogate pair in two; empty text is one empty piece (the erase and the caret move still go out).
fn split_units(units: &[u16], max: usize) -> Vec<&[u16]> {
    let mut pieces = Vec::new();
    let mut rest = units;
    while rest.len() > max {
        let at = if (0xD800..0xDC00).contains(&rest[max - 1]) { max - 1 } else { max };
        pieces.push(&rest[..at]);
        rest = &rest[at..];
    }
    pieces.push(rest);
    pieces
}

#[cfg(test)]
mod tests {
    use super::split_units;

    #[test]
    fn pieces_cover_the_text_and_keep_pairs_whole() {
        assert_eq!(split_units(&[], 4), vec![&[] as &[u16]]);
        assert_eq!(split_units(&[1, 2, 3], 4), vec![&[1u16, 2, 3][..]]);
        assert_eq!(split_units(&[1, 2, 3, 4, 5], 2), vec![&[1u16, 2][..], &[3, 4], &[5]]);
        // 0xD83D 0xDE00 is one character: the cut moves before its first half
        assert_eq!(split_units(&[1, 0xD83D, 0xDE00, 2], 2), vec![&[1u16][..], &[0xD83D, 0xDE00], &[2]]);
    }
}
