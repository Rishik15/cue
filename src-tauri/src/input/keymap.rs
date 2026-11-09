//! File: input/keymap.rs
//! Purpose: Turns what the Windows hook reports (virtual key, scan code, extended flag) into a ledger slot: a sided modifier or a main key. Punctuation is read by physical position (scan code) because its virtual key moves with the keyboard layout; everything else uses the stable virtual key. Pure, so it is tested off Windows.
//! Contents: slot — the mapping; is_phantom_ctrl — the Left Ctrl Windows invents around AltGr.

use super::key::{Key, Mods};
use super::ledger::Slot;

const OEM_VKS: [u16; 13] = [0xBA, 0xBB, 0xBC, 0xBD, 0xBE, 0xBF, 0xC0, 0xDB, 0xDC, 0xDD, 0xDE, 0xDF, 0xE2];
const ALTGR_PHANTOM_SCAN: u32 = 0x21D;
const LCONTROL: u16 = 0xA2;

/// Whether the event is the synthetic Left Ctrl Windows sends around every Right Alt on AltGr layouts; it carries no user intent.
pub fn is_phantom_ctrl(vk: u16, scan: u32) -> bool {
    vk == LCONTROL && scan == ALTGR_PHANTOM_SCAN
}

/// The key at a punctuation scan code (Scan Code Set 1, not extended), if it is one Cue knows.
fn punctuation(scan: u32) -> Option<Key> {
    Some(match scan {
        0x29 => Key::Grave,
        0x0C => Key::Minus,
        0x0D => Key::Equal,
        0x1A => Key::LeftBracket,
        0x1B => Key::RightBracket,
        0x2B => Key::Backslash,
        0x27 => Key::Semicolon,
        0x28 => Key::Quote,
        0x33 => Key::Comma,
        0x34 => Key::Period,
        0x35 => Key::Slash,
        0x56 => Key::Section,
        _ => return None,
    })
}

/// The modifier a virtual key is. The generic Shift counts as the left key; the generic Ctrl and Alt (VK_CONTROL, VK_MENU) are the right key when the event is extended and the left key otherwise, as Windows flags the right-hand ones.
fn modifier(vk: u16, extended: bool) -> Option<Mods> {
    Some(match vk {
        0xA0 | 0x10 => Mods::SHIFT_L,
        0xA1 => Mods::SHIFT_R,
        0xA2 => Mods::CTRL_L,
        0xA3 => Mods::CTRL_R,
        0x11 if extended => Mods::CTRL_R,
        0x11 => Mods::CTRL_L,
        0xA4 => Mods::ALT_L,
        0xA5 => Mods::ALT_R,
        0x12 if extended => Mods::ALT_R,
        0x12 => Mods::ALT_L,
        0x5B => Mods::WIN_L,
        0x5C => Mods::WIN_R,
        _ => return None,
    })
}

/// The slot a hook event is for, or None for a key Cue does not use.
pub fn slot(vk: u16, scan: u32, extended: bool) -> Option<Slot> {
    if let Some(m) = modifier(vk, extended) {
        return Some(Slot::Mod(m));
    }
    if !extended && OEM_VKS.contains(&vk) {
        if let Some(key) = punctuation(scan) {
            return Some(Slot::Key(key));
        }
    }
    let key = match vk {
        0x03 => Some(Key::Pause),
        0xDF => Some(Key::Grave),
        0x0D if extended => Some(Key::KeypadEnter),
        _ => Key::from_vk(vk),
    };
    key.map(Slot::Key)
}
