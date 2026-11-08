//! File: input/gesture.rs
//! Purpose: Decides, from the keys a recording hears, what the user is doing: showing keys, finishing a combination, or cancelling. Pure, so every key order can be replayed in a test. The only place that says when a combination is complete.
//! Contents: Ev — one key event as the recording sees it; Step — what it means; Gesture — the combination being pressed.

use super::chord::{Chord, Part};
use super::key::{Key, Mods};

/// One key event: the main key (None for a modifier), whether it went down, and the modifiers held after it.
#[derive(Clone, Copy, Debug)]
pub struct Ev {
    pub key: Option<Key>,
    pub down: bool,
    pub held: Mods,
}

/// What one event means for the recording.
#[derive(Debug, PartialEq)]
pub enum Step {
    Ignore,
    Live { parts: Vec<Part>, keyed: bool },
    Clear,
    Commit(Chord),
    Cancel,
}

/// The combination being pressed. One with a main key keeps the first chord pressed and completes when that key is released, even if the modifiers went up first; a modifier-only one completes when every modifier is up and it is usable. A bare Escape always cancels, even when a main key's release was lost.
#[derive(Default)]
pub struct Gesture {
    keyed: Option<Chord>,
    modifiers_only: Option<Chord>,
    shown: Option<Chord>,
}

impl Gesture {
    /// Forgets the combination so far, for a fresh try.
    pub fn reset(&mut self) {
        *self = Gesture::default();
    }

    /// Takes one event and says what it means.
    pub fn feed(&mut self, ev: Ev) -> Step {
        match ev.key {
            Some(key) => self.on_key(ev, key),
            None => self.on_modifier(ev),
        }
    }

    /// Says to show `chord` unless the window shows it already: repeats resend the same keys, and redrawing them makes the keycaps flicker.
    fn show(&mut self, chord: Chord, keyed: bool) -> Step {
        if self.shown == Some(chord) {
            return Step::Ignore;
        }
        self.shown = Some(chord);
        Step::Live { parts: chord.parts(), keyed }
    }

    /// A main key went down or up.
    fn on_key(&mut self, ev: Ev, key: Key) -> Step {
        if !ev.down {
            return match self.keyed {
                Some(chord) if chord.key == Some(key) => Step::Commit(chord),
                _ => Step::Ignore,
            };
        }
        if key == Key::Escape && ev.held.is_empty() {
            return Step::Cancel;
        }
        let Some(pressed) = Chord::new(ev.held, Some(key)) else { return Step::Ignore };
        let chord = *self.keyed.get_or_insert(pressed);
        self.show(chord, true)
    }

    /// A modifier went down or up: down shows the modifiers held; the last one up completes a modifier-only combination when it is usable, and a dropped tap clears the keycaps.
    fn on_modifier(&mut self, ev: Ev) -> Step {
        if self.keyed.is_some() {
            return Step::Ignore;
        }
        if ev.down {
            self.modifiers_only = Chord::new(ev.held, None);
            return self.modifiers_only.map_or(Step::Ignore, |chord| self.show(chord, false));
        }
        if !ev.held.is_empty() {
            return Step::Ignore;
        }
        let was_shown = self.shown.take().is_some();
        match self.modifiers_only.take() {
            Some(chord) if chord.usable() => Step::Commit(chord),
            _ if was_shown => Step::Clear,
            _ => Step::Ignore,
        }
    }
}
