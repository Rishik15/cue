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

#[cfg(test)]
mod tests {
    use super::*;
    use handy_keys::Modifiers;

    const CTRL: Modifiers = Modifiers::CTRL_LEFT;
    const ALT: Modifiers = Modifiers::OPT_LEFT;
    const SHIFT: Modifiers = Modifiers::SHIFT_LEFT;
    const WIN: Modifiers = Modifiers::CMD_LEFT;

    /// A modifier-only event: `modifiers` held after going down or up.
    fn modifier(modifiers: Modifiers, down: bool) -> KeyEvent {
        KeyEvent { modifiers, key: None, is_key_down: down, changed_modifier: None }
    }

    /// A key event for `key` going down or up with `modifiers` held.
    fn key(modifiers: Modifiers, key: Key, down: bool) -> KeyEvent {
        KeyEvent { modifiers, key: Some(key), is_key_down: down, changed_modifier: None }
    }

    /// Feeds `events` through a fresh gesture and returns every step it took.
    fn run(events: &[KeyEvent]) -> Vec<Step> {
        let mut gesture = Gesture::default();
        events.iter().map(|e| gesture.feed(e)).collect()
    }

    /// The combinations the steps asked to save, in order.
    fn commits(steps: &[Step]) -> Vec<Hotkey> {
        steps.iter().filter_map(|s| if let Step::Commit(h) = s { Some(*h) } else { None }).collect()
    }

    #[test]
    fn ctrl_k_commits_when_k_is_released() {
        let steps = run(&[modifier(CTRL, true), key(CTRL, Key::K, true), key(CTRL, Key::K, false), modifier(Modifiers::empty(), false)]);
        assert_eq!(commits(&steps), [Hotkey::new(CTRL, Key::K).unwrap()]);
        assert!(matches!(steps[2], Step::Commit(_)));
    }

    #[test]
    fn ctrl_alt_k_commits_with_both_modifiers() {
        let both = CTRL | ALT;
        let steps = run(&[modifier(CTRL, true), modifier(both, true), key(both, Key::K, true), key(both, Key::K, false)]);
        assert_eq!(commits(&steps), [Hotkey::new(both, Key::K).unwrap()]);
    }

    #[test]
    fn modifier_released_before_the_key_still_commits_the_chord() {
        let steps = run(&[
            modifier(CTRL, true),
            key(CTRL, Key::K, true),
            modifier(Modifiers::empty(), false),
            key(Modifiers::empty(), Key::K, false),
        ]);
        assert_eq!(commits(&steps), [Hotkey::new(CTRL, Key::K).unwrap()]);
        assert_eq!(steps[2], Step::Ignore);
    }

    #[test]
    fn repeats_and_other_keys_do_not_replace_the_chord() {
        let steps = run(&[
            modifier(CTRL, true),
            key(CTRL, Key::K, true),
            modifier(Modifiers::empty(), false),
            key(Modifiers::empty(), Key::K, true),
            key(Modifiers::empty(), Key::J, true),
            key(Modifiers::empty(), Key::J, false),
            key(Modifiers::empty(), Key::K, false),
        ]);
        assert_eq!(commits(&steps), [Hotkey::new(CTRL, Key::K).unwrap()]);
        assert_eq!(steps[4], Step::Ignore);
        assert_eq!(steps[5], Step::Ignore);
    }

    #[test]
    fn a_held_key_repeating_shows_the_keys_once() {
        let steps =
            run(&[modifier(CTRL, true), modifier(CTRL, true), key(CTRL, Key::K, true), key(CTRL, Key::K, true), key(CTRL, Key::K, true)]);
        let shown = steps.iter().filter(|s| matches!(s, Step::Live { .. })).count();
        assert_eq!(shown, 2);
    }

    #[test]
    fn the_same_modifier_pressed_again_after_a_dropped_tap_is_shown_again() {
        let mut gesture = Gesture::default();
        assert!(matches!(gesture.feed(&modifier(CTRL, true)), Step::Live { .. }));
        gesture.feed(&modifier(Modifiers::empty(), false));
        assert!(matches!(gesture.feed(&modifier(CTRL, true)), Step::Live { .. }));
    }

    #[test]
    fn win_k_commits() {
        let steps = run(&[modifier(WIN, true), key(WIN, Key::K, true), key(WIN, Key::K, false)]);
        assert_eq!(commits(&steps), [Hotkey::new(WIN, Key::K).unwrap()]);
    }

    #[test]
    fn modifier_only_commits_when_every_modifier_is_up() {
        let both = CTRL | SHIFT;
        let steps = run(&[modifier(CTRL, true), modifier(both, true), modifier(CTRL, false), modifier(Modifiers::empty(), false)]);
        assert_eq!(steps[2], Step::Ignore);
        assert_eq!(commits(&steps), [Hotkey::new(both, None).unwrap()]);
        assert!(matches!(steps[3], Step::Commit(_)));
    }

    #[test]
    fn a_lone_ctrl_tap_is_dropped_and_the_next_gesture_starts_clean() {
        let mut gesture = Gesture::default();
        assert!(matches!(gesture.feed(&modifier(CTRL, true)), Step::Live { keyed: false, .. }));
        assert_eq!(gesture.feed(&modifier(Modifiers::empty(), false)), Step::Ignore);
        gesture.feed(&modifier(ALT, true));
        assert!(matches!(gesture.feed(&modifier(Modifiers::empty(), false)), Step::Commit(_)));
    }

    #[test]
    fn bare_escape_cancels_but_ctrl_escape_is_a_chord() {
        assert_eq!(run(&[key(Modifiers::empty(), Key::Escape, true)]), [Step::Cancel]);
        let steps = run(&[modifier(CTRL, true), key(CTRL, Key::Escape, true), key(CTRL, Key::Escape, false)]);
        assert_eq!(commits(&steps), [Hotkey::new(CTRL, Key::Escape).unwrap()]);
        assert!(!steps.contains(&Step::Cancel));
    }

    #[test]
    fn escape_repeating_after_ctrl_was_released_does_not_cancel() {
        let steps = run(&[
            modifier(CTRL, true),
            key(CTRL, Key::Escape, true),
            modifier(Modifiers::empty(), false),
            key(Modifiers::empty(), Key::Escape, true),
        ]);
        assert!(!steps.contains(&Step::Cancel));
    }

    #[test]
    fn a_key_released_with_nothing_pressed_is_ignored() {
        assert_eq!(run(&[key(Modifiers::empty(), Key::K, false)]), [Step::Ignore]);
    }
}
