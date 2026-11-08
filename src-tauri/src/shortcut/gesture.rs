//! File: shortcut/gesture.rs
//! Purpose: Decides, from the stream of key events a recording hears, what the user is doing: showing keys, finishing a combination, or cancelling. Pure, so every key order can be replayed in a test. This is the only place that says when a combination is complete.
//! Contents: Step — what one event means; Gesture — the state of the combination being pressed, with `feed` and `reset`.

use handy_keys::{Hotkey, Key, KeyEvent};

use super::keys::{usable, view, Part};

/// What one key event means for the recording.
#[derive(Debug, PartialEq)]
pub(super) enum Step {
    /// Nothing to show or do.
    Ignore,
    /// Show these keys; `keyed` means a main key took part.
    Live { parts: Vec<Part>, keyed: bool },
    /// The combination is complete: validate and save it.
    Commit(Hotkey),
    /// A bare Escape: stop recording.
    Cancel,
}

/// The combination being pressed. A combination with a main key keeps the first chord pressed and completes when that key is released, even if the modifiers went up first; a modifier-only one completes when every modifier is up and it is usable.
#[derive(Default)]
pub(super) struct Gesture {
    keyed: Option<Hotkey>,
    modifiers_only: Option<Hotkey>,
    shown: Option<Hotkey>,
}

impl Gesture {
    /// Forgets the combination so far, for a fresh try after a refused one.
    pub(super) fn reset(&mut self) {
        *self = Gesture::default();
    }

    /// Takes one key event from the hook and says what it means. Input: the event as the hook saw it. Returns the step to act on.
    pub(super) fn feed(&mut self, ev: &KeyEvent) -> Step {
        match ev.key {
            Some(key) => self.on_key(ev, key),
            None => self.on_modifier(ev),
        }
    }

    /// Says to show `chord`, unless it is what the window shows already: key repeats resend the same keys, and redrawing them makes the keycaps flicker. Input: the chord and whether a main key took part. Returns `Live`, or `Ignore` for a repeat.
    fn show(&mut self, chord: Hotkey, keyed: bool) -> Step {
        if self.shown == Some(chord) {
            return Step::Ignore;
        }
        self.shown = Some(chord);
        Step::Live { parts: view(&chord), keyed }
    }

    /// A main key went down or up: the first chord pressed is kept (a repeat or another key does not replace it), a bare Escape cancels, the kept key's release completes it.
    fn on_key(&mut self, ev: &KeyEvent, key: Key) -> Step {
        if !ev.is_key_down {
            return match self.keyed {
                Some(chord) if chord.key == Some(key) => Step::Commit(chord),
                _ => Step::Ignore,
            };
        }
        if key == Key::Escape && ev.modifiers.is_empty() && self.keyed.is_none() {
            return Step::Cancel;
        }
        let Ok(pressed) = ev.as_hotkey() else { return Step::Ignore };
        let chord = *self.keyed.get_or_insert(pressed);
        self.show(chord, true)
    }

    /// A modifier went down or up: down shows the modifiers held; the last one up completes a modifier-only combination when it is usable (a lone Ctrl or Shift tap is dropped and the recording stays open).
    fn on_modifier(&mut self, ev: &KeyEvent) -> Step {
        if self.keyed.is_some() {
            return Step::Ignore;
        }
        if ev.is_key_down {
            self.modifiers_only = ev.as_hotkey().ok();
            return match self.modifiers_only {
                Some(chord) => self.show(chord, false),
                None => Step::Ignore,
            };
        }
        if !ev.modifiers.is_empty() {
            return Step::Ignore;
        }
        self.shown = None;
        match self.modifiers_only.take() {
            Some(chord) if usable(&chord) => Step::Commit(chord),
            _ => Step::Ignore,
        }
    }
}
