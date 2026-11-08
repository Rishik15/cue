//! File: input/key.rs
//! Purpose: The keys a shortcut may use, one table for name, label and Windows virtual key, and the sided modifier keys. Names are the exact strings settings.json already stores for a main key.
//! Contents: keys! — the table macro; Key — one main key (name, label, vk, lookup by name or vk); Mods — the nine modifier bits with today's stored values; Group — the four modifier families (Ctrl, Alt, Shift, Win).

use serde::{Deserialize, Deserializer, Serialize, Serializer};

macro_rules! keys {
    ($($id:ident $label:literal $vk:literal),* $(,)?) => {
        /// One main key a shortcut can use.
        #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
        #[repr(u8)]
        pub enum Key { $($id),* }

        const TABLE: &[(Key, &str, &str, u16)] = &[$((Key::$id, stringify!($id), $label, $vk)),*];
    };
}

keys! {
    A "A" 0x41, B "B" 0x42, C "C" 0x43, D "D" 0x44, E "E" 0x45, F "F" 0x46, G "G" 0x47, H "H" 0x48, I "I" 0x49,
    J "J" 0x4A, K "K" 0x4B, L "L" 0x4C, M "M" 0x4D, N "N" 0x4E, O "O" 0x4F, P "P" 0x50, Q "Q" 0x51, R "R" 0x52,
    S "S" 0x53, T "T" 0x54, U "U" 0x55, V "V" 0x56, W "W" 0x57, X "X" 0x58, Y "Y" 0x59, Z "Z" 0x5A,
    Num0 "0" 0x30, Num1 "1" 0x31, Num2 "2" 0x32, Num3 "3" 0x33, Num4 "4" 0x34,
    Num5 "5" 0x35, Num6 "6" 0x36, Num7 "7" 0x37, Num8 "8" 0x38, Num9 "9" 0x39,
    PlayPause "PlayPause" 0xB3, Stop "Stop" 0xB2, PrevTrack "PrevTrack" 0xB1, NextTrack "NextTrack" 0xB0,
    F1 "F1" 0x70, F2 "F2" 0x71, F3 "F3" 0x72, F4 "F4" 0x73, F5 "F5" 0x74, F6 "F6" 0x75,
    F7 "F7" 0x76, F8 "F8" 0x77, F9 "F9" 0x78, F10 "F10" 0x79, F11 "F11" 0x7A, F12 "F12" 0x7B,
    F13 "F13" 0x7C, F14 "F14" 0x7D, F15 "F15" 0x7E, F16 "F16" 0x7F, F17 "F17" 0x80, F18 "F18" 0x81,
    F19 "F19" 0x82, F20 "F20" 0x83, F21 "F21" 0x84, F22 "F22" 0x85, F23 "F23" 0x86, F24 "F24" 0x87,
    Space "Space" 0x20, Return "Return" 0x0D, Tab "Tab" 0x09, Escape "Escape" 0x1B,
    Delete "Delete" 0x08, ForwardDelete "ForwardDelete" 0x2E, Insert "Insert" 0x2D, Pause "Pause" 0x13,
    Home "Home" 0x24, End "End" 0x23, PageUp "PageUp" 0x21, PageDown "PageDown" 0x22,
    LeftArrow "Left" 0x25, RightArrow "Right" 0x27, UpArrow "Up" 0x26, DownArrow "Down" 0x28,
    Minus "-" 0xBD, Equal "=" 0xBB, LeftBracket "[" 0xDB, RightBracket "]" 0xDD, Backslash "\\" 0xDC,
    Semicolon ";" 0xBA, Quote "'" 0xDE, Comma "," 0xBC, Period "." 0xBE, Slash "/" 0xBF, Grave "`" 0xC0, Section "§" 0xE2,
    Keypad0 "Keypad0" 0x60, Keypad1 "Keypad1" 0x61, Keypad2 "Keypad2" 0x62, Keypad3 "Keypad3" 0x63,
    Keypad4 "Keypad4" 0x64, Keypad5 "Keypad5" 0x65, Keypad6 "Keypad6" 0x66, Keypad7 "Keypad7" 0x67,
    Keypad8 "Keypad8" 0x68, Keypad9 "Keypad9" 0x69, KeypadDecimal "KeypadDecimal" 0x6E,
    KeypadMultiply "KeypadMultiply" 0x6A, KeypadPlus "KeypadPlus" 0x6B, KeypadClear "KeypadClear" 0x0C,
    KeypadDivide "KeypadDivide" 0x6F, KeypadEnter "KeypadEnter" 0x0D, KeypadMinus "KeypadMinus" 0x6D,
    CapsLock "CapsLock" 0x14, ScrollLock "ScrollLock" 0x91, NumLock "NumLock" 0x90,
    PrintScreen "PrintScreen" 0x2C, ContextMenu "ContextMenu" 0x5D,
}

impl Key {
    pub const COUNT: usize = TABLE.len();

    /// The key's stored name (the variant name, as settings.json holds it).
    pub fn name(self) -> &'static str {
        TABLE[self as usize].1
    }

    /// What the Shortcuts page shows for the key.
    pub fn label(self) -> &'static str {
        TABLE[self as usize].2
    }

    /// The Windows virtual key that types this key on a US layout.
    pub fn vk(self) -> u16 {
        TABLE[self as usize].3
    }

    /// The key stored under `name`, or None for a name this build does not know.
    pub fn from_name(name: &str) -> Option<Key> {
        TABLE.iter().find(|row| row.1 == name).map(|row| row.0)
    }

    /// The key a virtual key code means (keypad Enter is told from Return by the caller), or None.
    pub fn from_vk(vk: u16) -> Option<Key> {
        TABLE.iter().find(|row| row.3 == vk && row.0 != Key::KeypadEnter).map(|row| row.0)
    }

    /// Whether this is F1 to F24.
    pub fn is_function(self) -> bool {
        self.name().strip_prefix('F').is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
    }
}

impl Serialize for Key {
    /// Writes the key as its name string.
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.name())
    }
}

impl<'de> Deserialize<'de> for Key {
    /// Reads a key name; an unknown name is an error so the row stays inactive with a visible message.
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Key, D::Error> {
        let name = String::deserialize(d)?;
        Key::from_name(&name).ok_or_else(|| serde::de::Error::custom(format!("unknown key {name}")))
    }
}

/// The modifier keys as bits; the values are the ones settings.json stores today, so a stored shortcut loads unchanged.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Mods(pub u32);

impl Mods {
    pub const NONE: Mods = Mods(0);
    pub const WIN_L: Mods = Mods(1 << 0);
    pub const SHIFT_L: Mods = Mods(1 << 1);
    pub const CTRL_L: Mods = Mods(1 << 2);
    pub const ALT_L: Mods = Mods(1 << 3);
    pub const FN: Mods = Mods(1 << 4);
    pub const WIN_R: Mods = Mods(1 << 5);
    pub const SHIFT_R: Mods = Mods(1 << 6);
    pub const CTRL_R: Mods = Mods(1 << 7);
    pub const ALT_R: Mods = Mods(1 << 8);
    pub const KNOWN: u32 = (1 << 9) - 1;

    /// Whether every bit of `other` is set.
    pub fn has(self, other: Mods) -> bool {
        self.0 & other.0 == other.0
    }

    /// Whether any bit of `other` is set.
    pub fn any(self, other: Mods) -> bool {
        self.0 & other.0 != 0
    }

    /// Whether no bit is set.
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The bits of both sets.
    pub fn with(self, other: Mods) -> Mods {
        Mods(self.0 | other.0)
    }

    /// These bits without the bits of `other`.
    pub fn without(self, other: Mods) -> Mods {
        Mods(self.0 & !other.0)
    }
}

impl std::ops::BitOr for Mods {
    type Output = Mods;

    /// The bits of both sets.
    fn bitor(self, rhs: Mods) -> Mods {
        self.with(rhs)
    }
}

/// One modifier family: its left bit, its right bit, its name and the part code the page draws.
#[derive(Clone, Copy)]
pub struct Group {
    pub left: Mods,
    pub right: Mods,
    pub code: &'static str,
    pub label: &'static str,
}

impl Group {
    /// Both side bits.
    pub fn both(&self) -> Mods {
        self.left | self.right
    }
}

pub const GROUPS: [Group; 4] = [
    Group { left: Mods::CTRL_L, right: Mods::CTRL_R, code: "ctrl", label: "Ctrl" },
    Group { left: Mods::ALT_L, right: Mods::ALT_R, code: "alt", label: "Alt" },
    Group { left: Mods::SHIFT_L, right: Mods::SHIFT_R, code: "shift", label: "Shift" },
    Group { left: Mods::WIN_L, right: Mods::WIN_R, code: "win", label: "Win" },
];
