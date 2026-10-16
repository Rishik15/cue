//! Purpose: Insert text into the focused app without taking focus (Windows).
//! Contents: press — a key several times; insert_slowly — type one character at a time; foreground_window — which window has focus; focus_window — bring one back to the front; insert / insert_rich / replace — short single-line text is typed (replace also erases what the user typed first and moves the caret, in one batch) as Unicode key events (clipboard untouched), anything else is
//! pasted through `paste` (also when the focused app's IME is in a native mode, where typed events would enter its composition); an elevated target is detected up front and the text is left on the clipboard instead.
//! key_chord / send — SendInput helpers shared with `paste`. Call from a worker thread: it sleeps while waiting for
//! modifiers and the target app.

use std::fmt;
use std::mem::size_of;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::HWND;
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId, IsIconic, SetForegroundWindow, ShowWindow, SW_RESTORE,
};

use super::{clipboard, elevation, ime, paste};
use crate::platform::{InsertOptions, Key};

const TYPE_MAX_UNITS: usize = 400;
/// The pause between characters of a slow insertion.
const SLOW_PAUSE: Duration = Duration::from_millis(8);
/// How long an insertion waits for the shortcut's keys to come up: releasing the main key first and the modifier a moment later is normal.
const MODIFIER_WAIT: Duration = Duration::from_secs(3);

/// One insertion at a time: two overlapping ones (back-to-back dictations) would fight over the clipboard and the keyboard queue.
static INSERTING: Mutex<()> = Mutex::new(());

#[derive(Debug)]
pub enum InsertError {
    /// The focused app runs as administrator; Windows blocks our input. The text was copied for a manual paste.
    Elevated,
    Failed(String),
}

impl fmt::Display for InsertError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            InsertError::Elevated => {
                f.write_str("This app runs as administrator, so Cue cannot type into it. The text is on your clipboard.")
            }
            InsertError::Failed(e) => write!(f, "Could not insert text: {e}"),
        }
    }
}

impl From<String> for InsertError {
    fn from(e: String) -> Self {
        InsertError::Failed(e)
    }
}

/// Identifies the window that has focus (0 when none), so dictated text is never typed into a different window than the one it was spoken into.
pub fn foreground_window() -> isize {
    unsafe { GetForegroundWindow().0 as isize }
}

/// Windows only lets a process take the foreground when it received the last input. Cue acts on a keyboard hook, which does not count, and a tiling
/// window manager moves windows around too, so a plain `SetForegroundWindow` is often ignored. Sharing the input queue of the current foreground thread
/// for the duration of the call makes the request legitimate; it is the standard workaround and needs no keystroke tricks.
fn force_foreground(window: HWND) {
    // SAFETY: plain Win32 calls on window handles; failures only mean the window stays where it was.
    unsafe {
        if IsIconic(window).as_bool() {
            let _ = ShowWindow(window, SW_RESTORE);
        }
        let (mine, theirs) = (GetCurrentThreadId(), GetWindowThreadProcessId(GetForegroundWindow(), None));
        let attached = theirs != 0 && theirs != mine && AttachThreadInput(mine, theirs, true).as_bool();
        let _ = BringWindowToTop(window);
        let _ = SetForegroundWindow(window);
        if attached {
            let _ = AttachThreadInput(mine, theirs, false);
        }
    }
}

/// Brings `window` to the front, gives it the keyboard focus and waits until Windows has done it (used to return to the app text was taken from, and to put the preview in front).
pub fn focus_window(window: isize) -> bool {
    force_foreground(HWND(window as _));
    let start = Instant::now();
    while foreground_window() != window && start.elapsed() < Duration::from_millis(500) {
        std::thread::sleep(Duration::from_millis(10));
    }
    foreground_window() == window
}

/// Presses Enter (or Ctrl+Enter) in the focused window, a moment after an insertion so the target has taken the text.
pub fn press_submit(with_ctrl: bool) -> Result<(), String> {
    std::thread::sleep(Duration::from_millis(150));
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
    let inputs: Vec<INPUT> = (0..count).flat_map(|_| [key(vk, 0, KEYBD_EVENT_FLAGS(0)), key(vk, 0, KEYEVENTF_KEYUP)]).collect();
    send(&inputs)
}

/// Types `text` one character at a time with a short pause between them, for sessions that drop fast input (remote desktops, virtual machines). Single-line text only: a line break typed this way would be an Enter.
pub fn insert_slowly(text: &str) -> Result<(), String> {
    let text = crate::sanitize::strip_hidden(text);
    let _one_at_a_time = INSERTING.lock().unwrap_or_else(|e| e.into_inner());
    if elevation::target_blocks_input() || !wait_modifiers_released(MODIFIER_WAIT, false) {
        return Err("Cue could not type here: the app runs as administrator or keys were still held down.".into());
    }
    let target = foreground_window();
    for unit in text.encode_utf16() {
        if foreground_window() != target {
            return Err("Focus moved to another window, so Cue stopped typing.".into());
        }
        send(&[key(VIRTUAL_KEY(0), unit, KEYEVENTF_UNICODE), key(VIRTUAL_KEY(0), unit, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP)])?;
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
            let why = match (kept, why.contains("your clipboard")) {
                (true, false) => format!("{why}, so the text is on your clipboard"),
                (false, _) => format!("{why}, and the text could not be saved to the clipboard"),
                _ => why,
            };
            Err(InsertError::Failed(why))
        }
        other => other,
    }
}

fn place(text: &str, html: Option<&str>, options: &InsertOptions, around: Around) -> Result<(), InsertError> {
    if elevation::target_blocks_input() {
        clipboard::set_plain_text(text)?;
        return Err(InsertError::Elevated);
    }
    if !wait_modifiers_released(MODIFIER_WAIT, around.erase > 0) {
        // Still holding Ctrl, Alt, Shift or Win: typed characters would run as shortcuts in the app. Leave the text for a manual paste.
        clipboard::set_plain_text(text)?;
        return Err(InsertError::Failed("keys were still held down, so the text is on your clipboard".into()));
    }
    let typeable = text.encode_utf16().count() <= TYPE_MAX_UNITS && !text.contains(['\n', '\r']) && !ime::native_input_active();
    if typeable && !options.always_paste && html.is_none() {
        return Ok(type_unicode(text, around)?);
    }
    if around.erase > 0 {
        press(Key::Backspace, around.erase)?;
    }
    paste::paste(text, html, options.restore_clipboard)?;
    if around.back > 0 {
        press(Key::Left, around.back)?;
    }
    Ok(())
}

/// Typed characters would turn into shortcuts (Ctrl+A...) while the dictation hotkey is still held. False when a key is still down after `max`. With `shift_ok` a held Shift does not count: typed Unicode characters
/// ignore it and Shift+Backspace still erases, so replacing a trigger the user has just typed (often with Shift) need not wait for the key to come up.
pub(super) fn wait_modifiers_released(max: Duration, shift_ok: bool) -> bool {
    let start = Instant::now();
    let watched: &[VIRTUAL_KEY] =
        if shift_ok { &[VK_CONTROL, VK_MENU, VK_LWIN, VK_RWIN] } else { &[VK_CONTROL, VK_SHIFT, VK_MENU, VK_LWIN, VK_RWIN] };
    let held = || watched.iter().any(|k| unsafe { GetAsyncKeyState(k.0 as i32) } as u16 & 0x8000 != 0);
    while held() && start.elapsed() < max {
        std::thread::sleep(Duration::from_millis(10));
    }
    !held()
}

fn key(vk: VIRTUAL_KEY, scan: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: vk, wScan: scan, dwFlags: flags, time: 0, dwExtraInfo: 0 } },
    }
}

/// Press and release `key` while `modifier` is held.
pub fn key_chord(modifier: VIRTUAL_KEY, k: VIRTUAL_KEY) -> [INPUT; 4] {
    let up = KEYEVENTF_KEYUP;
    [key(modifier, 0, KEYBD_EVENT_FLAGS(0)), key(k, 0, KEYBD_EVENT_FLAGS(0)), key(k, 0, up), key(modifier, 0, up)]
}

pub fn send(inputs: &[INPUT]) -> Result<(), String> {
    let sent = unsafe { SendInput(inputs, size_of::<INPUT>() as i32) };
    if sent as usize == inputs.len() {
        Ok(())
    } else {
        Err(format!("SendInput injected {sent}/{}", inputs.len()))
    }
}

/// Erases, types and moves the caret in one `SendInput` call: Windows keeps the events of one call together, so the user's own keys cannot come between them.
fn type_unicode(text: &str, around: Around) -> Result<(), String> {
    let tap = |vk: VIRTUAL_KEY| [key(vk, 0, KEYBD_EVENT_FLAGS(0)), key(vk, 0, KEYEVENTF_KEYUP)];
    let mut inputs: Vec<INPUT> = (0..around.erase).flat_map(|_| tap(VK_BACK)).collect();
    inputs.extend(
        text.encode_utf16()
            .flat_map(|u| [key(VIRTUAL_KEY(0), u, KEYEVENTF_UNICODE), key(VIRTUAL_KEY(0), u, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP)]),
    );
    inputs.extend((0..around.back).flat_map(|_| tap(VK_LEFT)));
    send(&inputs)
}
