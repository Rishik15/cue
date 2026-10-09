//! Purpose: Pure hotkey helpers: display parts for the UI and shortcut validation.
//! Contents: Part — one displayed key; default_hotkey — Left Ctrl + Space; view — hotkey to display parts;
//! usable — rejects bare non-F-keys.

use handy_keys::{Hotkey, Key, Modifiers};
use serde::Serialize;

/// One displayed key: `code` picks the icon, `side` is 'L'/'R' for side-specific modifiers.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Part {
    pub code: String,
    pub label: String,
    pub side: Option<char>,
}

pub fn default_hotkey() -> Hotkey {
    Hotkey::new(Modifiers::CTRL_LEFT, Key::Space).expect("valid default")
}

fn part(code: &str, label: &str, side: Option<char>) -> Part {
    Part { code: code.into(), label: label.into(), side }
}

/// Hotkey to display parts. Win+Shift+F23 is what the Copilot key sends, shown as one key.
pub fn view(h: &Hotkey) -> Vec<Part> {
    let m = h.modifiers;
    if m.intersects(Modifiers::CMD) && m.intersects(Modifiers::SHIFT) && h.key == Some(Key::F23) {
        return vec![part("copilot", "Copilot", None)];
    }
    let groups = [
        ("ctrl", "Ctrl", Modifiers::CTRL_LEFT, Modifiers::CTRL_RIGHT),
        ("alt", "Alt", Modifiers::OPT_LEFT, Modifiers::OPT_RIGHT),
        ("shift", "Shift", Modifiers::SHIFT_LEFT, Modifiers::SHIFT_RIGHT),
        ("win", "Win", Modifiers::CMD_LEFT, Modifiers::CMD_RIGHT),
    ];
    let mut parts = Vec::new();
    for (code, label, l, r) in groups {
        match (m.contains(l), m.contains(r)) {
            (true, false) => parts.push(part(code, label, Some('L'))),
            (false, true) => parts.push(part(code, label, Some('R'))),
            (true, true) => parts.push(part(code, label, None)),
            _ => {}
        }
    }
    if m.contains(Modifiers::FN) {
        parts.push(part("fn", "Fn", None));
    }
    if let Some(k) = h.key {
        let label = k.to_string();
        parts.push(part(&label.to_lowercase(), &label, None));
    }
    parts
}

fn is_function_key(k: Key) -> bool {
    let s = k.to_string();
    s.len() > 1 && s.starts_with('F') && s[1..].chars().all(|c| c.is_ascii_digit())
}

/// A bare non-F-key would hijack normal typing.
pub fn usable(h: &Hotkey) -> bool {
    !h.modifiers.is_empty() || h.key.is_some_and(is_function_key)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codes(h: &str) -> Vec<(String, Option<char>)> {
        view(&h.parse::<Hotkey>().unwrap()).into_iter().map(|p| (p.code, p.side)).collect()
    }

    #[test]
    fn view_and_validation() {
        assert_eq!(codes("Ctrl+Space"), [("ctrl".into(), None), ("space".into(), None)]);
        assert_eq!(view(&default_hotkey())[0].side, Some('L'));
        let left = Hotkey::new(Modifiers::CTRL_LEFT, Key::Grave).unwrap();
        assert_eq!(view(&left)[0].side, Some('L'));
        assert_eq!(view(&left)[1].code, "`");
        let copilot = Hotkey::new(Modifiers::CMD_LEFT | Modifiers::SHIFT_LEFT, Key::F23).unwrap();
        assert_eq!(view(&copilot)[0].code, "copilot");
        assert!(usable(&Hotkey::new(Modifiers::empty(), Key::F9).unwrap()));
        assert!(usable(&Hotkey::new(Modifiers::OPT_LEFT, None).unwrap())); // modifier-only
        assert!(!usable(&Hotkey::new(Modifiers::empty(), Key::A).unwrap()));
    }
}
