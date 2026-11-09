//! File: win/input/inject.rs
//! Purpose: Cue's own keystrokes: every key carries `MARK` so the keyboard hook ignores it, batches go out through SendInput with an exact count, and `Lift` keeps the shape other modules already use on top of the input module's lift transaction.
//! Contents: MARK — the marker; send_keys — a batch out, the count the system took; send_menu_mask — the inert key tap that keeps Alt or Win from opening a menu, true when both events were taken (the pump calls it inside the hook, before the release passes); os_modifiers — the modifiers Windows reports down; lift_blocked — the focused app is elevated; Lift — lift on new (an error when it cannot be made), restore on drop.

use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP,
    KEYEVENTF_UNICODE, VIRTUAL_KEY,
};

use crate::input::{KeyBatch, KeyStep, LiftGuard, Mods};

/// The marker (dwExtraInfo, "CUEI") on every key Cue injects; the hook passes marked keys without looking at them.
pub const MARK: usize = 0x4355_4549;

const MENU_MASK_VK: u16 = 0xE8;

/// The side-specific modifier keys and the bit each one is.
const MODIFIER_KEYS: [(Mods, i32); 8] = [
    (Mods::WIN_L, 0x5B),
    (Mods::WIN_R, 0x5C),
    (Mods::SHIFT_L, 0xA0),
    (Mods::SHIFT_R, 0xA1),
    (Mods::CTRL_L, 0xA2),
    (Mods::CTRL_R, 0xA3),
    (Mods::ALT_L, 0xA4),
    (Mods::ALT_R, 0xA5),
];

/// The system event for one step, marked as Cue's own.
fn input_for(step: &KeyStep) -> INPUT {
    let mut flags = if step.up { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) };
    if step.extended {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    let (vk, scan) = if step.unicode {
        flags |= KEYEVENTF_UNICODE;
        (VIRTUAL_KEY(0), step.vk)
    } else {
        (VIRTUAL_KEY(step.vk), step.scan)
    };
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: vk, wScan: scan, dwFlags: flags, time: 0, dwExtraInfo: MARK } },
    }
}

/// Sends the batch and returns how many events the system took. Safety: the array is built here and lives for the call.
pub fn send_keys(batch: &KeyBatch) -> usize {
    let inputs: Vec<INPUT> = batch.0.iter().map(input_for).collect();
    unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) as usize }
}

/// Presses and releases the unassigned key the shell ignores, once.
pub fn send_menu_mask() -> bool {
    let step = |up| KeyStep { vk: MENU_MASK_VK, scan: 0, up, extended: false, unicode: false };
    send_keys(&KeyBatch(vec![step(false), step(true)])) == 2
}

/// The modifiers Windows reports down now. Safety: `GetAsyncKeyState` only reads the key state for a virtual-key code.
pub fn os_modifiers() -> Mods {
    MODIFIER_KEYS
        .iter()
        .filter(|(_, vk)| (unsafe { GetAsyncKeyState(*vk) }) as u16 & 0x8000 != 0)
        .fold(Mods::NONE, |all, (bit, _)| all | *bit)
}

/// Whether the focused app is elevated, so injected keys would be dropped.
pub fn lift_blocked() -> bool {
    super::super::elevation::target_blocks_input()
}

/// While alive, the modifiers the user holds are lifted; dropping it puts back the ones still held.
pub struct Lift(Option<LiftGuard>);

impl Lift {
    /// Lifts the held modifiers. Err with a line for the user when they cannot be lifted: keys sent with a modifier still held would be a different shortcut, so the caller sends nothing.
    pub fn new() -> Result<Lift, String> {
        crate::input::lift().map(|guard| Lift(Some(guard))).map_err(|e| format!("Cue could not release your held keys first: {e}"))
    }
}

impl Drop for Lift {
    /// Restores the modifiers; a failure is logged.
    fn drop(&mut self) {
        if let Some(guard) = self.0.take() {
            if let Err(e) = guard.finish() {
                crate::log::note(&format!("input: restoring the modifiers failed: {e}"));
            }
        }
    }
}
