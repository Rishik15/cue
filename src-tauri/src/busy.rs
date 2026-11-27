//! File: busy.rs
//! Purpose: One answer to "is something already using the screen or the keyboard?" for every shortcut press, so two features never read or type at once (a second Ctrl+C from the wrong window, a card over the palette).
//! Contents: busy — dictating, the palette, the strip or the question window open, a lookup or card up, a selection or grab in progress; busy_with — which of those it is, for a log line; first_busy — the probes in order, stopping at the first busy one.

use tauri::AppHandle;

use crate::{dictation, dictionary, grab, preview, selection, snippets, strip};

/// Whether a feature already uses the screen or keyboard. Input: app handle. Returns true when a shortcut press must be ignored.
pub fn busy(app: &AppHandle) -> bool {
    busy_with(app).is_some()
}

/// The first feature that already uses the screen or keyboard. Input: app handle. Returns its name, or None when nothing does.
pub fn busy_with(app: &AppHandle) -> Option<&'static str> {
    first_busy([
        ("dictation", &dictation::is_recording),
        ("preview", &|| preview::is_open(app)),
        ("strip", &|| strip::is_open(app)),
        ("lookup", &dictionary::is_looking),
        ("selection", &selection::is_busy),
        ("grab", &grab::is_busy),
        ("snippet question", &snippets::ask::is_open),
    ])
}

/// The name of the first probe that answers busy. Input: named probes in order. Return: that name, or None.
fn first_busy<const N: usize>(probes: [(&'static str, &dyn Fn() -> bool); N]) -> Option<&'static str> {
    probes.into_iter().find_map(|(name, probe)| probe().then_some(name))
}

#[cfg(test)]
mod tests {
    use super::first_busy;
    use std::cell::Cell;

    /// The probes run in order and stop at the first busy one, so a later probe (a window lookup, a lock) is never evaluated after a match.
    #[test]
    fn probes_stop_at_the_first_busy_feature() {
        let later = Cell::new(0);
        let count = || {
            later.set(later.get() + 1);
            false
        };
        assert_eq!(first_busy([("a", &|| false), ("b", &|| true), ("c", &count), ("d", &count)]), Some("b"));
        assert_eq!(later.get(), 0);
        assert_eq!(first_busy([("a", &|| false), ("c", &count)]), None);
        assert_eq!(later.get(), 1);
    }
}
