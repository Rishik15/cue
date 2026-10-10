//! Purpose: Insert text into the focused app without taking focus (Windows).
//! Contents: foreground_window — which window has focus; insert — short single-line text is typed as Unicode key events (clipboard untouched), anything else is
//! pasted through `paste`; an elevated target is detected up front and the text is left on the clipboard instead.
//! key_chord / send — SendInput helpers shared with `paste`. Call from a worker thread: it sleeps while waiting for
//! modifiers and the target app.

use std::fmt;
use std::mem::size_of;
use std::time::{Duration, Instant};

use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

use super::{clipboard, elevation, paste};
use crate::platform::InsertOptions;

const TYPE_MAX_UNITS: usize = 400;

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

pub fn insert(text: &str, options: &InsertOptions) -> Result<(), InsertError> {
    if elevation::target_blocks_input() {
        clipboard::set_plain_text(text)?;
        return Err(InsertError::Elevated);
    }
    wait_modifiers_released(Duration::from_millis(600));
    let typeable = text.encode_utf16().count() <= TYPE_MAX_UNITS && !text.contains(['\n', '\r']);
    if typeable && !options.always_paste {
        Ok(type_unicode(text)?)
    } else {
        Ok(paste::paste(text, options.restore_clipboard)?)
    }
}

/// Typed characters would turn into shortcuts (Ctrl+A...) while the dictation hotkey is still held.
fn wait_modifiers_released(max: Duration) {
    let start = Instant::now();
    let held =
        || [VK_CONTROL, VK_SHIFT, VK_MENU, VK_LWIN, VK_RWIN].iter().any(|k| unsafe { GetAsyncKeyState(k.0 as i32) } as u16 & 0x8000 != 0);
    while held() && start.elapsed() < max {
        std::thread::sleep(Duration::from_millis(10));
    }
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

fn type_unicode(text: &str) -> Result<(), String> {
    let inputs: Vec<INPUT> = text
        .encode_utf16()
        .flat_map(|u| [key(VIRTUAL_KEY(0), u, KEYEVENTF_UNICODE), key(VIRTUAL_KEY(0), u, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP)])
        .collect();
    send(&inputs)
}
