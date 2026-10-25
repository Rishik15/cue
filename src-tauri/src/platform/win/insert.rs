//! Purpose: Insert text into the focused app without taking focus (Windows).
//! Contents: insert_slowly — erase, then type one character at a time; insert / replace_rich / replace — short single-line text (up to 100 characters, espanso's limit) is typed as Unicode key events in one `SendInput` batch with the erase and the caret move (replace erases what the user typed first and moves the caret), anything else is
//! pasted through `paste` (also when the focused app's IME is in a native mode, where typed events would enter its composition, and on a weak or remote machine where a typed burst is dropped — `fragile_env`); an elevated target is detected up front and the text is left on the clipboard instead.
//! key_chord / send — SendInput helpers shared with `paste`; every key is marked as Cue's own and the shortcut's held modifiers are lifted around it (`input::Lift`), so nothing waits for the user to let go. `press_submit` and the inserts take the same one-insertion-at-a-time lock as the inserts, and an insert holds one `Lift` around erase, paste and caret move. Call from a worker thread: it sleeps while waiting for the target app.

use std::fmt;
use std::mem::size_of;
use std::sync::Mutex;
use std::time::Duration;

use windows::Win32::UI::Input::KeyboardAndMouse::*;

use super::focus::foreground_window;
use super::input::Lift;
use super::{clipboard, elevation, ime, paste};
use crate::platform::InsertOptions;

/// Text longer than this is pasted, not typed (espanso's `clipboard_threshold`).
const TYPE_MAX_UNITS: usize = 100;
/// Slow insertion, per character: how long the key stays down and the pause after it, the same on every machine (a pace tuned to the hardware it was measured on fails on the next one).
const SLOW_HOLD: Duration = Duration::from_millis(5);
const SLOW_PAUSE: Duration = Duration::from_millis(15);
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

/// Erases `erase` characters, types `text` one character at a time with a short pause between them and moves the caret `back` to the left, for sessions that drop fast input (remote desktops, virtual machines).
/// All of it happens under one lock and one lift, and the erase goes out right before the first character. Single-line text only: a line break typed this way would be an Enter.
pub fn insert_slowly(erase: usize, text: &str, back: usize) -> Result<(), String> {
    let text = crate::sanitize::strip_hidden(text);
    let _one_at_a_time = INSERTING.lock().unwrap_or_else(|e| e.into_inner());
    if elevation::target_blocks_input() {
        return Err("Cue could not type here: the app runs as administrator.".into());
    }
    let _lift = Lift::new(); // held shortcut keys must not turn the typed characters into shortcuts
    let target = foreground_window();
    send(&taps(VK_BACK, erase))?;
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
    send(&taps(VK_LEFT, back))
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

/// Replaces the last `erase` typed characters with `text` pasted with `html` (a fragment) beside it, so apps that take formatting get it and the rest get the text; then moves the caret `back`.
pub fn replace_rich(erase: usize, text: &str, html: &str, back: usize, options: &InsertOptions) -> Result<(), InsertError> {
    deliver(text, Some(html), options, Around { erase, back })
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
    let typeable = fits_typing(text) && !ime::native_input_active() && !fragile_env();
    if typeable && !options.always_paste && html.is_none() {
        let _lift = Lift::new();
        return Ok(type_unicode(text, around)?);
    }
    let _lift = Lift::new(); // one lift around erase, paste and caret move
    paste::paste(text, html, options.restore_clipboard, around.erase)?;
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

#[cfg(test)]
mod tests {
    use super::fits_typing;

    #[test]
    fn typing_is_for_short_single_line_text() {
        assert!(fits_typing(&"a".repeat(100)));
        assert!(!fits_typing(&"a".repeat(101)));
        assert!(!fits_typing("two\nlines"));
        assert!(fits_typing(&"😀".repeat(50)) && !fits_typing(&"😀".repeat(51)));
        // counted in UTF-16 units
    }
}
