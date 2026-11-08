//! File: insert/keys.rs
//! Purpose: The SendInput layer of text delivery: keyboard events marked as Cue's own, single keys and chords, and the one call that sends a batch and balances it when Windows takes only part of it.
//! Contents: Short — the counts of a short batch; key / taps / key_chord — build events; try_send / send — inject them (a short batch is balanced by `unreleased`, which key-ups every key the injected part pressed).

use std::fmt;
use std::mem::size_of;

use windows::Win32::UI::Input::KeyboardAndMouse::*;

/// `SendInput` injected fewer events than given.
#[derive(Debug)]
pub struct Short {
    pub sent: u32,
    pub total: usize,
}

impl fmt::Display for Short {
    /// The count that went out, for the status line.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "SendInput injected {}/{}", self.sent, self.total)
    }
}

/// A keyboard event marked as Cue's own, so the keyboard hook ignores it.
pub(in crate::platform::win) fn key(vk: VIRTUAL_KEY, scan: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT { wVk: vk, wScan: scan, dwFlags: flags, time: 0, dwExtraInfo: crate::platform::win::input::MARK },
        },
    }
}

/// Press and release `k` alone, `count` times. Arrow and navigation keys are sent as extended keys: without that flag Windows reads an arrow as its numpad twin, so a `{cursor}` caret move (Left) types a digit when NumLock is on.
pub fn taps(k: VIRTUAL_KEY, count: usize) -> Vec<INPUT> {
    let extended = matches!(k, VK_LEFT | VK_RIGHT | VK_UP | VK_DOWN | VK_HOME | VK_END | VK_PRIOR | VK_NEXT | VK_INSERT | VK_DELETE);
    let down = if extended { KEYEVENTF_EXTENDEDKEY } else { KEYBD_EVENT_FLAGS(0) };
    (0..count).flat_map(|_| [key(k, 0, down), key(k, 0, down | KEYEVENTF_KEYUP)]).collect()
}

/// Press and release `key` while `modifier` is held.
pub fn key_chord(modifier: VIRTUAL_KEY, k: VIRTUAL_KEY) -> [INPUT; 4] {
    let up = KEYEVENTF_KEYUP;
    [key(modifier, 0, KEYBD_EVENT_FLAGS(0)), key(k, 0, KEYBD_EVENT_FLAGS(0)), key(k, 0, up), key(modifier, 0, up)]
}

/// Sends the inputs in one `SendInput` call; Err with the counts when Windows injected fewer than given. A short batch is balanced at once: every key the injected part pressed and did not release gets its key-up (`unreleased`), so a half-sent Ctrl+V cannot leave Ctrl or V down. The counts stay those of the batch. Safety: `inputs` is a valid slice of fully initialised `INPUT` values (each keyboard union member written by `key`), and the size passed is that of one `INPUT`.
pub fn try_send(inputs: &[INPUT]) -> Result<(), Short> {
    if inputs.is_empty() {
        return Ok(());
    }
    let sent = unsafe { SendInput(inputs, size_of::<INPUT>() as i32) };
    if sent as usize == inputs.len() {
        return Ok(());
    }
    let releases = unreleased(inputs, sent as usize);
    if !releases.is_empty() {
        unsafe { SendInput(&releases, size_of::<INPUT>() as i32) };
    }
    Err(Short { sent, total: inputs.len() })
}

/// The key-ups that balance the first `sent` inputs: every key pressed there and not released, latest first. A key is the same virtual key, scan code and unicode/extended flags.
fn unreleased(inputs: &[INPUT], sent: usize) -> Vec<INPUT> {
    let mut down: Vec<(VIRTUAL_KEY, u16, KEYBD_EVENT_FLAGS)> = Vec::new();
    for input in inputs.iter().take(sent) {
        let ki = unsafe { input.Anonymous.ki };
        let flags = ki.dwFlags & (KEYEVENTF_UNICODE | KEYEVENTF_EXTENDEDKEY);
        let id = (ki.wVk, ki.wScan, flags);
        if ki.dwFlags.contains(KEYEVENTF_KEYUP) {
            if let Some(at) = down.iter().rposition(|d| *d == id) {
                down.remove(at);
            }
        } else {
            down.push(id);
        }
    }
    down.into_iter().rev().map(|(vk, scan, flags)| key(vk, scan, flags | KEYEVENTF_KEYUP)).collect()
}

/// `try_send` with the counts as a sentence.
pub fn send(inputs: &[INPUT]) -> Result<(), String> {
    try_send(inputs).map_err(|short| short.to_string())
}

#[cfg(test)]
mod tests {
    use super::{key_chord, taps, unreleased};
    use windows::Win32::UI::Input::KeyboardAndMouse::{VK_CONTROL, VK_LEFT, VK_V};

    /// The (virtual key, key-up) of each input, for reading a batch in a test.
    fn shape(inputs: &[windows::Win32::UI::Input::KeyboardAndMouse::INPUT]) -> Vec<(u16, bool)> {
        use windows::Win32::UI::Input::KeyboardAndMouse::KEYEVENTF_KEYUP;
        inputs.iter().map(|i| unsafe { i.Anonymous.ki }).map(|k| (k.wVk.0, k.dwFlags.contains(KEYEVENTF_KEYUP))).collect()
    }

    /// A Ctrl+V batch that stopped after Ctrl-down and V-down is balanced by V-up then Ctrl-up; one that stopped after Ctrl-down releases Ctrl; a whole or an empty one needs nothing; a pressed arrow is released with the same flags.
    #[test]
    fn a_short_batch_releases_every_key_it_pressed() {
        let chord = key_chord(VK_CONTROL, VK_V);
        assert_eq!(shape(&unreleased(&chord, 2)), vec![(VK_V.0, true), (VK_CONTROL.0, true)]);
        assert_eq!(shape(&unreleased(&chord, 1)), vec![(VK_CONTROL.0, true)]);
        assert!(unreleased(&chord, 0).is_empty());
        assert!(unreleased(&chord, 4).is_empty());
        assert!(unreleased(&chord, 3).len() == 1);
        let arrows = taps(VK_LEFT, 2);
        let released = unreleased(&arrows, 3);
        assert_eq!(shape(&released), vec![(VK_LEFT.0, true)]);
        assert_eq!(unsafe { released[0].Anonymous.ki }.dwFlags, unsafe { arrows[1].Anonymous.ki }.dwFlags);
    }
}
