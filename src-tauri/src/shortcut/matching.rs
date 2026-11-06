//! Purpose: Per-event matching for the hotkey thread (hold-to-talk) and the rules a new combination must pass.
//! Contents: step_match — turns key events into press/release of each shortcut and hands them to the actions thread, and Escape cancels what Cue is doing; release_pressed — lets go of every shortcut held down; overlaps — a modifier-only shortcut inside a longer one; validate — whether a recorded combination may be saved.

use handy_keys::{Hotkey, Key, KeyEvent, Modifiers};

use super::actions::Actions;
use super::keys::usable;
use super::{label, Binding, N};

/// Matching mode: each shortcut's press and release go to its feature (dictation also emits `dictation-shortcut` for the settings window).
pub fn step_match(actions: &Actions, bindings: &[Binding], ev: &KeyEvent, pressed: &mut [bool; N]) {
    // Escape is never blocked, so the app in front still sees it; Cue just stops what it was doing. Modifiers do not matter: in Hold mode
    // the dictation keys are still down when Escape is pressed. When nothing is being done this is a few atomic reads.
    if ev.is_key_down && ev.key == Some(Key::Escape) {
        crate::dictation::cancel();
        crate::selection::cancel();
        crate::dictionary::cancel();
    }
    // Typing in the window a dictionary card points at moves the text on: the card goes. Keys of Cue's own shortcuts do not count.
    if ev.is_key_down && ev.key.is_some_and(|k| bindings.iter().all(|b| b.active().is_none_or(|h| h.key != Some(k)))) {
        crate::dictionary::dismiss_on_typing();
    }
    for (b, was_down) in bindings.iter().zip(pressed.iter_mut()) {
        // A shortcut the user turned off, or a feature turned off, matches nothing; let go of it first if it was somehow still counted down.
        let Some(h) = b.active() else {
            if std::mem::take(was_down) {
                dispatch(actions, b.name, false);
            }
            continue;
        };
        let down = match (h.key, ev.key) {
            (Some(k), Some(ek)) if k == ek => {
                ev.is_key_down && (h.modifiers.matches(ev.modifiers) || *was_down && still_held(&h, ev.modifiers))
            }
            (Some(_), _) => *was_down && still_held(&h, ev.modifiers),
            (None, None) => h.modifiers.matches(ev.modifiers) || *was_down && still_held(&h, ev.modifiers),
            (None, Some(_)) => *was_down,
        };
        if down != *was_down {
            *was_down = down;
            dispatch(actions, b.name, down);
        }
    }
}

/// Once a shortcut is down it stays down for as long as its own keys are held: touching another modifier meanwhile (Shift, or Alt on the
/// way to the Selection shortcut) must not read as letting go. Starting still needs the exact modifiers, so Ctrl+Shift+Space is not Ctrl+Space.
fn still_held(h: &Hotkey, held: Modifiers) -> bool {
    let wanted = GROUPS.into_iter().filter(|g| h.modifiers.intersects(*g)).fold(Modifiers::empty(), |all, g| all | g);
    h.modifiers.matches(held & wanted)
}

const GROUPS: [Modifiers; 5] = [Modifiers::CMD, Modifiers::SHIFT, Modifiers::CTRL, Modifiers::OPT, Modifiers::FN];

/// True when one shortcut is modifier-only and its modifiers are all among the other's: pressing the longer combo passes through the shorter one, which would fire too.
pub(super) fn overlaps(a: &Hotkey, b: &Hotkey) -> bool {
    let within = |small: &Hotkey, big: &Hotkey| {
        small.key.is_none() && GROUPS.into_iter().filter(|g| small.modifiers.intersects(*g)).all(|g| big.modifiers.intersects(g))
    };
    within(a, b) || within(b, a)
}

/// Sends the release of every shortcut counted as down (a bug, or recording starting, would otherwise leave it recording forever).
pub fn release_pressed(actions: &Actions, bindings: &[Binding], pressed: &mut [bool; N]) {
    for (b, was_down) in bindings.iter().zip(pressed.iter_mut()) {
        if std::mem::take(was_down) {
            dispatch(actions, b.name, false);
        }
    }
}

/// Whether `h` may become `name`'s shortcut: usable on its own, not another shortcut's, not overlapping one. Returns the message to show when it may not.
pub(super) fn validate(bindings: &[Binding], name: &str, h: &Hotkey) -> Result<(), String> {
    if !usable(h) {
        return Err("Add a modifier key (Ctrl, Alt, Shift, Win) or use an F-key. A modifier alone needs two, or Alt or Win.".into());
    }
    if let Some(other) = bindings.iter().find(|b| b.name != name && b.hotkey == *h) {
        return Err(format!("Already used by {}.", label(other.name)));
    }
    if let Some(other) = bindings.iter().find(|b| b.name != name && overlaps(&b.hotkey, h)) {
        return Err(format!("Overlaps {}.", label(other.name)));
    }
    Ok(())
}

/// Hands a press or release to the actions thread; the hotkeys thread never waits for a feature to start.
fn dispatch(actions: &Actions, name: &'static str, down: bool) {
    let _ = actions.send((name, down));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extra_modifiers_do_not_release_a_held_shortcut() {
        let ctrl_space = Hotkey::new(Modifiers::CTRL_LEFT, Key::Space).unwrap();
        assert!(still_held(&ctrl_space, Modifiers::CTRL_LEFT));
        assert!(still_held(&ctrl_space, Modifiers::CTRL_LEFT | Modifiers::OPT_LEFT));
        assert!(!still_held(&ctrl_space, Modifiers::OPT_LEFT)); // its own modifier went up
        assert!(!still_held(&ctrl_space, Modifiers::CTRL_RIGHT)); // the left one is required
    }

    #[test]
    fn a_modifier_only_shortcut_overlaps_longer_combos_containing_it() {
        let hk = |m, k: Option<Key>| Hotkey::new(m, k).unwrap();
        let ctrl_alt = hk(Modifiers::CTRL | Modifiers::OPT, None);
        let ctrl_alt_space = hk(Modifiers::CTRL | Modifiers::OPT, Some(Key::Space));
        assert!(overlaps(&ctrl_alt, &ctrl_alt_space) && overlaps(&ctrl_alt_space, &ctrl_alt));
        assert!(!overlaps(&ctrl_alt, &hk(Modifiers::CTRL | Modifiers::SHIFT, None)));
        assert!(!overlaps(&hk(Modifiers::CTRL, Some(Key::Space)), &ctrl_alt_space));
        // neither is modifier-only
    }
}
