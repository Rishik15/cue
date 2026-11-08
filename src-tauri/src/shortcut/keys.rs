//! Purpose: Pure hotkey helpers: display parts for the UI and shortcut validation.
//! Contents: Part — one displayed key; view — hotkey to display parts; usable — rejects bare non-F-keys and a lone Ctrl, Shift or Fn. Shortcut names and defaults live in `registry`.

use handy_keys::{Hotkey, Key, Modifiers};
use serde::Serialize;

/// One displayed key: `code` picks the icon, `side` is 'L'/'R' for side-specific modifiers.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Part {
    pub code: String,
    pub label: String,
    pub side: Option<char>,
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

/// A bare non-F-key would hijack normal typing. Modifiers alone fire on every Ctrl or Shift tap unless there are two of them, or one is Alt or Win.
pub fn usable(h: &Hotkey) -> bool {
    if h.key.is_some() {
        return !h.modifiers.is_empty() || h.key.is_some_and(is_function_key);
    }
    let kinds = [Modifiers::CTRL, Modifiers::SHIFT, Modifiers::OPT, Modifiers::CMD, Modifiers::FN];
    kinds.iter().filter(|k| h.modifiers.intersects(**k)).count() >= 2 || h.modifiers.intersects(Modifiers::OPT | Modifiers::CMD)
}
